# AGENTS.md

> This document serves as a navigation guide for the AI and automated agents working within this repository. It summarizes the overall architecture of the repository, the responsibilities of each crate in the Cargo workspace, the boundaries of submodules under the main `app/` binary, and the engineering conventions that must be observed before making changes.
>
> This document works in tandem with `WARP.md`: `WARP.md` is the developer's handbook (covering commands, coding style, and workflows), while this document is the **code map**. Read `WARP.md` first, then refer to this document to locate the correct crate or module.

---

## 1. Repository Overview

Warp is a Rust-centric **agentic terminal and development environment**: it integrates terminal emulation, AI agents, cloud sync (Drive), code reviews, completions, Notebooks, settings, IPC, and more on top of a custom UI framework (WarpUI).

Top-level directories:

| Directory | Purpose |
|------|------|
| `app/` | Main binary crate (`warp`), assembling all subsystems, UI, database migrations, and platform glue layers |
| `crates/` | 67 workspace members, library crates split by responsibility |
| `command-signatures-v2/` | Independent subproject (excluded by `--exclude` when nextest runs) |
| `script/` | Cross-platform bootstrap, build, and presubmit scripts |
| `resources/` | Runtime resources such as fonts, icons, shell integration scripts, shaders, etc. |
| `docker/` | Containerized build configuration |
| `specs/` | Product and technical spec documents |
| `.agents/skills`, `.claude/skills` | Skill descriptions for agent workflows (creating PRs, fixing errors, feature rollouts, etc.) |
| `.warp/`, `.config/`, `.cargo/`, `.vscode/` | Various tool configurations |

Build system: Cargo workspace, `resolver = "2"`. The `default-members` are intentionally restricted to the subset that requires frequent compilation/testing (see `Cargo.toml`). `serve-wasm` and `integration` are excluded from `default-members` by default.

License division:
- `crates/warpui` and `crates/warpui_core` → MIT
- Everything else → AGPL-3.0-only

---

## 2. Top-level Architecture Layering

Roughly 4 layers from bottom to top. When adding new code or locating a bug, first determine which layer the change belongs to, and **never introduce reverse dependencies across layers**.

```
app/  (Main binary: assembly, entry points, platform glue, persistence migration, UI view root)
  ↑
Product domain crates: ai / computer_use / vim / onboarding /
              warp_completer / lsp / languages / code-review …
  ↑
Framework crates: warpui / warpui_core / warpui_extras / editor /
            ui_components / sum_tree / syntax_tree
  ↑
Infrastructure crates: warp_core / warp_util / http_client /
                websocket / ipc / jsonrpc / persistence / graphql /
                managed_secrets / virtual_fs / watcher / asset_cache …
```

Key architectural patterns (see `WARP.md` for details):

1. **Entity-Handle System**: `App` globally owns all view/model entities. Views reference each other via `ViewHandle<T>` rather than direct ownership.
2. **Element / Action**: The UI consists of a declarative Element tree + Action event system (Flutter-style).
3. **Cross-platform**: Native implementations for macOS / Windows / Linux + WASM targets; platform code is isolated with `#[cfg(...)]`.
4. **AI Integration**: Agent Mode and context indexing, code is concentrated in `app/src/ai` (389 files) and `crates/ai`.
5. **Cloud Sync**: `Drive` allows objects to sync across multiple devices, see `app/src/drive` and `crates/warp_files`.
6. **Feature Flags**: Runtime rollouts are preferred over `#[cfg]`, enums are defined in `crates/warp_core/src/features.rs`.

---

## 3. Overview of `crates/`

The table below lists all 67 crates grouped by theme. Each row has a **one-sentence responsibility**; for implementation details, open the corresponding `crates/<name>/src/lib.rs` directly (many crates have `//!` module documentation at the top of `lib.rs`).

### 3.1 UI Framework / View Layer

| Crate | Responsibility |
|-------|------|
| `warpui_core` | Core of the WarpUI framework (MIT): infrastructure like `App`, `Entity`, `ViewHandle`, and `AppContext` |
| `warpui` | Upper-level WarpUI components, Element tree, layout, and rendering pipeline (MIT) |
| `warpui_extras` | Optional extensions for WarpUI, not all features are enabled by default |
| `ui_components` | High-level reusable component library across views (buttons, inputs, lists, modals, etc.) |
| `editor` (`warp_editor`) | Text editor: buffers, selection, cursors, keymaps, undo stack |
| `sum_tree` | Persistent balanced B-tree, core data structure for the editor / Notebook / large lists |
| `syntax_tree` | Tree-sitter wrapper and syntax highlighting support |
| `markdown_parser` | Markdown parsing (used for AI messages, doc views, Notebooks, etc.) |
| `vim` | Vim mode keybindings and operational semantics |
| `voice_input` | Voice input support |

### 3.2 Terminal

| Crate | Responsibility |
|-------|------|
| `warp_terminal` | Core of terminal emulation: PTY management, ANSI/VT parsing, grid, scrolling, and shell integration hooks |
| `input_classifier` | Terminal input intent classification (pure command / natural language / AI Prompt) |
| `natural_language_detection` | Natural language recognition (in coordination with `input_classifier`) |

### 3.3 AI / Agent

| Crate | Responsibility |
|-------|------|
| `ai` | AI model client, Prompt orchestration, Agent protocol, and tool invocation framework |
| `computer_use` | Rust-side implementation of "Computer Use" tool capabilities (screenshots, clicks, typing, etc.) |
| `command-signatures-v2` | Command signatures v2 (metadata for AI command classification); independent project, excluded from workspace tests |
| `onboarding` | Data and state for new user onboarding flows |

### 3.4 Network / Protocols / IPC

| Crate | Responsibility |
|-------|------|
| `http_client` | Unified HTTP client wrapper in the workspace |
| `http_server` | Embedded HTTP server (local RPC, login callbacks, etc.) |
| `websocket` | Shared WebSocket abstraction for native and WASM, adapted to `graphql_ws_client` |
| `graphql` (`warp_graphql`) | GraphQL client, query/subscription generation, built from `graphql/api/schema.graphql` |
| `warp_graphql_schema` | GraphQL server schema definitions / shared types |
| `warp_server_client` | High-level RPC client interacting with the warp server |
| `ipc` | Generic typed IPC request/response protocol (between processes) |
| `jsonrpc` | JSON-RPC implementation |
| `lsp` | Language Server Protocol client implementation |
| `remote_server` | Server-side logic under remote sshd mode |
| `serve-wasm` | Auxiliary server to host WASM build artifacts (excluded from compilation by default) |
| `firebase` | Firebase client tools (crash reporting/analytics channels, etc.) |

### 3.5 Persistence / Files / Resources

| Crate | Responsibility |
|-------|------|
| `persistence` | Diesel + SQLite persistence layer foundation; **migrations are in `app/migrations/`, schema in `app/src/persistence/schema.rs`** |
| `warp_files` | Synchronizable file objects like Drive files, Workflows, and Notebooks |
| `virtual_fs` | Abstract file system (mock FS for testing and real FS for production share the same interface) |
| `repo_metadata` | Repository metadata: file tree construction, `.gitignore` handling, and file system watching |
| `watcher` | File system watcher (wrapper around `notify`) |
| `asset_cache` | Disk/memory cache for resources |
| `asset_macro` | Resource referencing macros such as `bundled!` / `theme!` |
| `managed_secrets` / `managed_secrets_wasm` | Keychain / DPAPI / Linux Keyring abstraction + WASM proxy |

### 3.6 Configuration / Settings

| Crate | Responsibility |
|-------|------|
| `settings` | Settings storage and change distribution |
| `settings_value` | `SettingsValue` trait: controls TOML serialization semantics |
| `settings_value_derive` | `#[derive(SettingsValue)]` procedural macro (converts enum variants to snake_case, etc.) |
| `warp_features` | High-level Feature Flag API (consumer side) |
| `channel_versions` | Release channels (stable/preview/dogfood) and version comparison |

### 3.7 Commands / Completions / Languages

| Crate | Responsibility |
|-------|------|
| `command` | Safe process spawning wrapper across platforms, **specifically handles Windows `no_window` flags**; all new child processes must use this |
| `warp_completer` | Completion engine (supports `--features v2`) |
| `languages` | Registration of languages, extensions, and Tree-sitter grammars |
| `warp_ripgrep` | Thin wrapper around ripgrep used by `warp_cli` |
| `warp_cli` | CLI subcommand parser within the binary (`warp <subcmd>`) |
| `fuzzy_match` | Fuzzy matching + glob-style wildcard matching, used for path search and command palettes |

### 3.8 Platform / System Services

| Crate | Responsibility |
|-------|------|
| `app-installation-detection` | Detects installed applications in the system (for launcher integration) |
| `prevent_sleep` | Inhibits sleep (during long-running tasks/AI Agent runs) |
| `isolation_platform` | Compatibility layer for running in sandboxes like Docker / GitHub Actions |
| `node_runtime` | Automatically installs/manages Node.js and npm (macOS/Linux/Windows × multi-arch) |
| `warp_js` | Helper abstraction for manipulating JavaScript values/functions on the Rust side |

### 3.9 Common Utilities / Communication

| Crate | Responsibility |
|-------|------|
| `warp_core` | The lowest "core" layer in the workspace: platform abstractions, the `FeatureFlag` enum in `features.rs` and `DOGFOOD/PREVIEW/RELEASE_FLAGS` |
| `warp_util` | General utility functions shared across multiple crates |
| `warp_logging` | Unified entry point for logging configuration |
| `simple_logger` | Simple asynchronous file logger for stderr-only processes like `remote_server` |
| `warp_web_event_bus` | Web-side event bus (for embedded web views) |
| `field_mask` | gRPC/Proto style FieldMask utility |
| `string-offset` | Offset base types (byte/char/utf16) |
| `handlebars` | Handlebars template engine wrapper |
| `integration` | Integration testing framework, used only for testing |

> **Naming quirks**: the package name for `crates/editor` is `warp_editor`; `crates/graphql` is `warp_graphql`; `crates/isolation_platform` is `warp_isolation_platform`; `crates/managed_secrets` is `warp_managed_secrets`; `crates/virtual_fs` is `virtual-fs` (hyphen); `crates/string-offset` is `string-offset` (hyphen).

---

## 4. `app/` Submodule Navigation

Under `app/src/`, there are 60+ flat product domain directories. Each directory corresponds to a product feature line. Below is a grouped summary by theme, with the approximate number of `.rs` files in parentheses to estimate complexity:

### 4.1 Startup / Assembly / Global
- `bin/` (7) — Multiple binary entry points (main program, accompanying tools).
- `lib.rs` / `app_state.rs` / `app_state_tests.rs` — Application state root.
- `app_menus.rs`, `app_services/`, `app_id_test.rs`
- `appearance.rs`, `gpu_state.rs`, `font_fallback.rs`, `global_resource_handles.rs`
- `dynamic_libraries.rs`, `alloc.rs`, `tracing.rs`, `profiling.rs`
- `crash_recovery.rs`, `crash_reporting/` (4)
- `features.rs` — Consumption of `warp_core::FeatureFlag` within `app/`; adding a flag typically requires wiring it in both places.
- `channel.rs`, `download_method.rs`, `autoupdate/` (8)

### 4.2 Terminal
- `terminal/` (427) — Core: shell processes, PTY, grid, blocks, shell integration, command execution, and I/O pipeline.
- `default_terminal/` (2) — Default terminal startup logic.
- `shell_indicator.rs`, `prefix.rs` / `prefix_test.rs` (command prefix parsing), `vim_registers.rs`

### 4.3 AI / Agent
- `ai/` (389) — Contains Agent UI, conversation models, Agent management, tools/MCP, Cloud Agent, Plan/Diff views, artifacts, blocklists, execution profiles, etc. **This is the largest subtree in the repository**. Before making changes, grep for specific subthemes (`agent_*`, `conversation_*`, `cloud_agent_*`, `mcp`, `tool_*`) within this directory.
- `ai_assistant/` (9) — Legacy AI assistant entry point/adaptation.
- `chip_configurator/`, `context_chips/` (22) — Selection and construction of Agent context chips.
- `coding_entrypoints/` (5), `coding_panel_enablement_state.rs`
- `prompt/` (2), `tips/` (3), `voice/` (2), `completer/` (3)

### 4.4 Editor / Code / Review
- `editor/` (38) — Main editor integration.
- `code/` (52) — Code views, diffs, and navigation.
- `code_review/` (36) — Code review workflow.
- `notebooks/` (30), `workflows/` (22)

### 4.5 Search
- `search/` (172) — Multi-target search (files, commands, Agent history, etc.).
- `search_bar.rs`

### 4.6 Server Communication / Drive / Sync
- `server/` (55) — HTTP/WS interaction with the warp backend (corresponds to local development mode `with_local_server`).
- `drive/` (45) — Entry point for cloud object sync.
- `cloud_object/` (12) — Cloud object abstraction layer (workflows, notebooks, etc.).
- `remote_server/` (5) — Client-side glue for connecting to the remote mode sshd.

### 4.7 Settings / User Configuration / Themes / Onboarding
- `settings/` (46), `settings_view/` (63)
- `user_config/` (6), `themes/` (11), `appearance.rs`
- `experiments/` (7), `tab_configs/` (15), `launch_configs/` (4)
- `tips/`, `banner/` (3), `quit_warning/` (1), `wasm_nux_dialog.rs`, `referral_theme_status.rs`

### 4.8 Auth / Billing / Usage
- `auth/` (22) — Login, tokens, and SSO.
- `billing/` (3), `pricing/` (1), `usage/` (1), `reward_view.rs`

### 4.9 Persistence
- `persistence/` (9) — Diesel migrations assembly, `schema.rs` (generated by Diesel), and migration runner.
- Migration files are located in the top-level `migrations/` directory of the repository (managed by Diesel CLI).

### 4.10 Platform / System Integration
- `platform/` (2), `system/` (3) / `system.rs`
- `login_item/` (3), `antivirus/` (3), `network.rs`
- `external_secrets/` (1), `env_vars/` (14)
- `keyboard.rs` / `keyboard_test.rs`, `safe_triangle.rs` / `safe_triangle_tests.rs` (safe hover triangle for menus)

### 4.11 View Root / Panels / Common UI
- `root_view.rs` / `root_view_tests.rs`
- `pane_group/` (35) — Split layout and pane management.
- `tab.rs`, `command_palette.rs`, `modal.rs`, `menu.rs` / `menu_test.rs`
- `palette.rs`, `notification.rs`, `resource_center/` (10)
- `view_components/` (20), `ui_components/` (14)
- `workspace/` (54), `workspaces/` (10), `voltron.rs` (coordination of multiple windows/workspaces)
- `session_management.rs`, `undo_close/` (3), `word_block_editor.rs`
- `suggestions/` (2), `input_suggestions.rs` / `input_suggestions_test.rs`
- `plugin/` (21) — Plugin system integration.
- `uri/` (7) — `warp://` URL handling.
- `debug_dump.rs`, `debounce.rs`, `interval_timer.rs`, `throttle.rs`
- `linear.rs`, `resource_limits.rs`, `warp_managed_paths_watcher.rs`
- `preview_config_migration.rs` / `preview_config_migration_tests.rs`
- `window_settings.rs`, `projects.rs`

### 4.12 Test Infrastructure
- `integration_testing/` (79) — End-to-end integration test support.
- `test_util/` (6) — Public utilities for unit testing.

---

## 5. Engineering Discipline (Hard Constraints for Agents)

> These are derived from `WARP.md` and custom project rules. Violating them will lead to PR rejection or runtime bugs.

### 5.1 Critical Conventions
- **Comments and replies must all be in English** (User Rule).
- For searching/grepping within git-indexed files, use the `fff` tool or `rg -n "<keyword>" <path>`; `read_file` is reserved for image/binary files.
- Before submitting a PR or pushing a new commit, you **MUST** run: `cargo fmt` + `cargo clippy --workspace --all-targets --all-features --tests -- -D warnings` + `./script/presubmit`.
- Changes must be precise: **every line of modification must be traceable to a user request**. Do not modify unrelated code, comments, or formatting on the side.
- Simplicity first: do not introduce abstractions, configurations, error handling, or redundant features for single-use scenarios.
- Explain options and expose uncertainties rather than silently making decisions on behalf of the user.

### 5.2 Rust Coding Style (from `WARP.md`)
- Do not write redundant type annotations for closure parameters.
- Keep `use` statements grouped at the top of the file; do not write long path qualifiers except inside `#[cfg]` branches.
- Name the context parameter `ctx` and place it last; if there are also closure parameters, place the closure last.
- Remove unused parameters **completely** instead of prefixing them with `_`. Update all call sites synchronously.
- Use inline format arguments in macros like `println!` / `format!` (e.g., `"{x}"` instead of `"{}" , x`) to satisfy the `uninlined_format_args` lint.
- **Wildcard `_` matches are strictly prohibited** in `match` statements (unless absolutely necessary) to ensure exhaustive matching.
- Do not delete or modify existing comments unless the logic they describe has changed.

### 5.3 Terminal Model Lock (High Priority!)
- Calling `TerminalModel::lock()` can easily cause deadlocks (manifesting as frozen UI or beachballing).
- Before calling `model.lock()`, verify that no parent caller in the current call stack already holds the lock. Prefer passing already-locked references down the call stack instead of locking again.
- Minimize the scope of the lock, and do not call functions that might attempt to lock the model while holding a lock.

### 5.4 Feature Flags
- Adding flags: add a variant in the `FeatureFlag` enum in `crates/warp_core/src/features.rs`, and add it to `DOGFOOD_FLAGS` / `PREVIEW_FLAGS` / `RELEASE_FLAGS` as needed.
- Usage: **prefer** runtime checks like `FeatureFlag::Xxx.is_enabled()` over `#[cfg(...)]`; use `#[cfg]` only when compilation is impossible without it (e.g., platform-specific dependencies).
- Wrap entire feature areas instead of checking flags at every single call site. **Clean up flags and dead branches** once the feature is stable.
- The UI entry point and code paths must be gated by the same flag.

### 5.5 Database
- ORM: Diesel + SQLite.
- Schema modifications must run via migrations: add a new directory under `migrations/` containing `up.sql` / `down.sql`. Do not manually modify `app/src/persistence/schema.rs` (which is generated by `diesel print-schema`).

### 5.6 Testing
- Run tests with `cargo nextest run --no-fail-fast --workspace --exclude command-signatures-v2`.
- Place unit tests in `${filename}_tests.rs` or `mod_test.rs`, and include them at the end of the original file with:

  ```rust
  #[cfg(test)]
  #[path = "filename_tests.rs"]
  mod tests;
  ```

- Integration tests use the `crates/integration` framework; examples are in `app/src/integration_testing/`.

### 5.7 Cross-process Spawning
- Never spawn child processes directly using `std::process::Command::new(...)` (which can pop up terminal windows on Windows). Use the wrapper in `crates/command` instead.

### 5.8 Sub-agents / Multi-agents
- Split large tasks into parallel sub-agents with **non-overlapping write domains**. Information gathering tasks can also run in parallel.
- Execute simple tasks directly without over-splitting.

---

## 6. Quick Reference for Common Entry Points

| Intended Action | Starting Point |
|---------|------|
| Modify terminal grid / shell integration | `crates/warp_terminal/src/`, coordinated with `app/src/terminal/` |
| Modify Agent UI / Conversations | Grep by theme (`agent_*` / `conversation_*`) under `app/src/ai/` |
| Modify command completion | `crates/warp_completer/` (note `--features v2`) |
| Modify AI model / Tool invocation protocol | `crates/ai/` |
| Add new settings | `crates/settings_value*`, `crates/settings`, UI in `app/src/settings_view/` |
| Add Feature Flags | `crates/warp_core/src/features.rs` + usage sites |
| Modify cloud sync objects | `crates/warp_files` + `app/src/drive/` + `app/src/cloud_object/` |
| Add GraphQL operations | Add to `crates/graphql/`, schema in `graphql/api/schema.graphql` |
| Modify persistence structure | Add migration to `migrations/` + `crates/persistence` |
| Add a new binary tool | `app/src/bin/` |
| Platform-specific code | Use `#[cfg(target_os = "...")]`, UI platform glue is in `app/src/platform/` |
| Vim mode | `crates/vim` + `app/src/vim_registers.rs` |
| Notebook / Workflow | `app/src/notebooks/`, `app/src/workflows/`, `crates/warp_files` |
| Cross-platform process spawning | `crates/command` |
| File search / Watching | `crates/repo_metadata`, `crates/watcher`, `crates/warp_ripgrep` |

---

## 7. Checklist Before Modification

Before picking up the keyboard to modify code, ask yourself:

1. Which layer / crate / `app/src/<submodule>` does this belong to? Will this change cross layer boundaries?
2. Is a new dependency required? If an existing workspace dependency can be reused, prioritize reusing it from the `[workspace.dependencies]` in `Cargo.toml`.
3. Is this a product feature? Does it need to be gated behind a Feature Flag?
4. Does this involve the terminal model? Does the current call stack already hold the `TerminalModel` lock?
5. Does this involve a child process? Does it use `crates/command`?
6. Does this involve persistence? Is a migration needed?
7. Have you written corresponding tests in `${file}_tests.rs`?
8. Are `cargo fmt` / `cargo clippy -D warnings` / relevant `cargo nextest` runs passing cleanly?
9. Can every single line of modification be mapped back to a user request? Should "small refactorings" done on the side be rolled back?

Double check all 9 items before delivering.
