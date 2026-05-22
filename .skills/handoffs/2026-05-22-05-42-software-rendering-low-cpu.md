# Handoff: Software Rendering Low-CPU Work

## 1. Context

We are working in `/mnt/Data/Apps/warp`, a Warp fork designed to run without VRAM by forcing software rendering (`llvmpipe`/CPU rendering). The user reported high CPU usage even when only a CLI waiting animation/dots/spinner is running in the terminal. Goal: reduce CPU usage without using VRAM.

No project-root `CONTEXT.md` exists, so domain terms come from `AGENTS.md` and codebase structure:

- WarpUI: custom UI framework under `crates/warpui` and `crates/warpui_core`.
- Winit event loop / windowing: `crates/warpui/src/windowing/winit/event_loop/mod.rs`, `window.rs`.
- Terminal model/view: `app/src/terminal/model/terminal_model.rs`, `app/src/terminal/view.rs`.
- Software rendering path: recent fork commits force/configure software rendering and frame pacing.
- Demand-driven rendering: terminal model has atomic `generation`; terminal view uses `last_rendered_generation` in `handle_terminal_wakeup`.

Plan target: 7/10 complexity, not full renderer rewrite. Implement adaptive software frame pacing + terminal spinner/output-shape batching + animation throttles + measurement.

## 2. Current State

Branch:

- Current branch: `feature/software-low-cpu-rendering`

Committed in this session:

1. `6eff5c2 docs(rendering): plan software low cpu mode`
2. `bf0b4d7 feat(rendering): add software frame policy`
3. `881f5b3 feat(rendering): adapt software frame pacing`

Files created:

- `/mnt/Data/Apps/warp/.skills/plans/software-rendering-low-cpu/PLAN.md`
  - Full 26-task implementation plan for low-CPU software rendering.
- `/mnt/Data/Apps/warp/.skills/handoffs/2026-05-22-05-42-software-rendering-low-cpu.md`
  - This handoff.

Files modified and committed:

- `/mnt/Data/Apps/warp/crates/warpui/src/windowing/winit/event_loop/mod.rs`
  - Added `SoftwareFrameClass` and `SoftwareFramePolicy`.
  - Added env-backed FPS policy using:
    - `WARP_SOFTWARE_FPS`
    - `WARP_SOFTWARE_IDLE_FPS`
    - `WARP_SOFTWARE_TERMINAL_FPS`
    - `WARP_SOFTWARE_INTERACTIVE_FPS`
    - `WARP_SOFTWARE_BURST_FPS`
  - Default software FPS classes:
    - idle: no interval/self-render
    - terminal steady: 6 FPS (`166ms`)
    - interactive: 12 FPS (`83ms`)
    - burst: 30 FPS (`33ms`)
  - Added `current_frame_class`, `burst_until`, `current_frame_interval`, `start_burst`, `set_frame_class`, and event classification.
  - `RedrawRequested` now uses `current_frame_interval()` instead of fixed `min_frame_interval`.
  - Added tests under `software_frame_policy_tests`.

Files modified but NOT committed:

- `/mnt/Data/Apps/warp/app/src/terminal/model/terminal_model_test.rs`
  - 32 inserted lines: tests added for future output-shape classifier:
    - `classifies_carriage_return_spinner_output_as_ephemeral`
    - `classifies_backspace_spinner_output_as_ephemeral`
    - `classifies_large_output_as_bulk`
    - `classifies_newline_heavy_output_as_bulk`
  - These tests currently do not compile because `TerminalOutputShape` and `classify_terminal_output_shape` do not exist yet.

Untracked:

- `/mnt/Data/Apps/warp/.codegraph/`
  - CodeGraph index, do not commit unless repo wants it. Existing `git status` shows it untracked.

Deleted files:

- None.

Staged changes:

- None known. Run `git status --short` to confirm.

Background process note:

- A stuck `cargo test -p warp classifies_carriage_return_spinner_output_as_ephemeral -- --nocapture` process was killed before writing this handoff. Verify with `ps -eo pid,etime,cmd | grep -E 'cargo|rustc' | grep -v grep`.

## 3. What Works

Verified and committed:

- CodeGraph setup completed and index up to date:
  - 3,371 files, 98,048 nodes, 123,749 edges.
- Plan file committed:
  - `.skills/plans/software-rendering-low-cpu/PLAN.md`
- WarpUI software frame policy tests passed:
  - Command run:
    - `cargo test -p warpui software_frame_policy_tests -- --nocapture`
  - Result:
    - 5 tests passed.
- Commits `bf0b4d7` and `881f5b3` are on branch `feature/software-low-cpu-rendering`.

NVIDIA/Gemini subagent findings:

- Tested NVIDIA pi models:
  - `nvidia/qwen/qwen3-coder-480b-a35b-instruct` passed smoke test.
  - `nvidia/mistralai/mistral-large-3-675b-instruct-2512` passed smoke test.
  - `nvidia/nvidia/llama-3.1-nemotron-ultra-253b-v1` returned `404 status code (no body)`.
  - `nvidia/deepseek-ai/deepseek-v4-pro` timed out.
- User said Gemini can be used as fallback if NVIDIA is slow/failing.

## 4. What Is In Progress

In progress: Task 8/20 from plan — terminal output shape classification.

Current uncommitted RED tests in `/mnt/Data/Apps/warp/app/src/terminal/model/terminal_model_test.rs` expect:

```rust
classify_terminal_output_shape(b"\r.\r..") == TerminalOutputShape::EphemeralSameLine
classify_terminal_output_shape(b"loading\x08\x08") == TerminalOutputShape::EphemeralSameLine
classify_terminal_output_shape(&[b'x'; 4096]) == TerminalOutputShape::Bulk
classify_terminal_output_shape(b"one\ntwo\nthree\n") == TerminalOutputShape::Bulk
```

These tests currently fail at compile stage because the classifier and enum are not implemented. This is intentional TDD RED state.

Caveat: Running `cargo test -p warp ...` may hit unrelated compile/test issues in `app` (one observed error: `error[E0061]: this function takes 5 arguments but 3 arguments were supplied`, plus unrelated warnings). Need inspect full output if it persists; earlier output was truncated due repeated runs/file lock.

## 5. Next Steps

1. Check working tree and processes:
   ```bash
   git status --short
   ps -eo pid,etime,cmd | grep -E 'cargo|rustc' | grep -v grep || true
   ```
   If cargo process remains from previous run, kill only that exact test process.

2. Continue TDD for output-shape classifier:
   - Open `/mnt/Data/Apps/warp/app/src/terminal/model/terminal_model.rs`.
   - Add near `TerminalModel` or generation helpers:
     ```rust
     #[derive(Clone, Copy, Debug, PartialEq, Eq)]
     pub enum TerminalOutputShape {
         Unknown,
         Bulk,
         EphemeralSameLine,
     }

     pub fn classify_terminal_output_shape(bytes: &[u8]) -> TerminalOutputShape {
         if bytes.is_empty() {
             return TerminalOutputShape::Unknown;
         }

         if bytes.len() >= 4096 || bytes.iter().filter(|byte| **byte == b'\n').count() >= 2 {
             return TerminalOutputShape::Bulk;
         }

         if bytes.len() <= 64 && bytes.iter().any(|byte| matches!(*byte, b'\r' | 0x08)) {
             return TerminalOutputShape::EphemeralSameLine;
         }

         TerminalOutputShape::Bulk
     }
     ```
   - Prefer exact project style; no `as any` equivalent, no broad refactor.

3. Run the focused tests:
   ```bash
   cargo test -p warp classifies_carriage_return_spinner_output_as_ephemeral -- --nocapture
   cargo test -p warp classifies_backspace_spinner_output_as_ephemeral -- --nocapture
   cargo test -p warp classifies_large_output_as_bulk -- --nocapture
   cargo test -p warp classifies_newline_heavy_output_as_bulk -- --nocapture
   ```
   If `cargo test -p warp` fails from unrelated existing errors, capture exact full failure with a bounded command and decide whether to use narrower test target or document blocker.

4. When tests pass, run:
   ```bash
   rustfmt app/src/terminal/model/terminal_model.rs app/src/terminal/model/terminal_model_test.rs
   cargo test -p warp classifies_carriage_return_spinner_output_as_ephemeral -- --nocapture
   ```

5. Commit classifier tests + implementation:
   ```bash
   git add app/src/terminal/model/terminal_model.rs app/src/terminal/model/terminal_model_test.rs
   git commit -m "feat(terminal): classify spinner output shape"
   ```

6. Implement model state fields after classifier commit:
   - File: `/mnt/Data/Apps/warp/app/src/terminal/model/terminal_model.rs`
   - Add fields to `TerminalModel`:
     ```rust
     last_output_shape: TerminalOutputShape,
     last_output_shape_generation: u64,
     ```
   - Initialize in `TerminalModel::mock` and main constructor/initializer near `generation` initialization.
   - Add getters:
     ```rust
     pub fn output_shape(&self) -> TerminalOutputShape { self.last_output_shape }
     pub fn output_shape_generation(&self) -> u64 { self.last_output_shape_generation }
     ```

7. Wire classifier in `on_finish_byte_processing(&mut self, input: &ansi::ProcessorInput<'_>)`:
   - Existing code around line ~3124:
     ```rust
     fn on_finish_byte_processing(&mut self, input: &ansi::ProcessorInput<'_>) {
         self.increment_generation();
         ...
         let bytes = input.bytes();
     ```
   - Change to get `bytes` before/after as needed, classify, then set generation:
     ```rust
     let bytes = input.bytes();
     self.last_output_shape = classify_terminal_output_shape(bytes);
     self.last_output_shape_generation = self.generation();
     ```
   - Be careful: if you call `increment_generation()` before `generation()`, generation value changes. Pick consistent semantics and test.

8. Add tests for model state if practical:
   - Process bytes through existing `Processor` as seen near synchronized output tests.
   - Assert `terminal.output_shape()` after spinner bytes and bulk bytes.

9. Commit model-state wiring:
   ```bash
   git add app/src/terminal/model/terminal_model.rs app/src/terminal/model/terminal_model_test.rs
   git commit -m "feat(terminal): track output shape for wakeup batching"
   ```

10. Next major work after model tracking: terminal view batching in `/mnt/Data/Apps/warp/app/src/terminal/view.rs`:
    - Add fields to `TerminalView`:
      ```rust
      last_software_terminal_notify: Option<Instant>,
      pending_software_terminal_notify: bool,
      ```
    - Locate `handle_terminal_wakeup` around line ~7894.
    - If software low-power mode and `model.output_shape() == TerminalOutputShape::EphemeralSameLine`, skip `ctx.notify()` until next configured terminal frame interval.
    - Do not set `last_rendered_generation` when skipping, so next allowed notify sees latest state.
    - Add a single pending timer/task; avoid spawning unbounded timers.

11. Add/centralize software mode helper:
    - Candidate file: `/mnt/Data/Apps/warp/app/src/settings/gpu.rs`
    - Helper behavior:
      - enabled by `WARP_SOFTWARE_LOW_POWER=1`, `WARP_FORCE_SOFTWARE=1`, or `LIBGL_ALWAYS_SOFTWARE=1`.
      - env override FPS for terminal batching should match `WARP_SOFTWARE_TERMINAL_FPS`, default 6.

12. Run checks after each commit:
    ```bash
    cargo fmt
    cargo test -p warpui software_frame_policy_tests -- --nocapture
    cargo test -p warp <new-test-name> -- --nocapture
    ```
    Avoid `cargo build` unless user approves (repo rule says avoid build/dev unless needed/ask first).

13. Before final handoff/PR, sync CodeGraph:
    ```bash
    npx -y @colbymchenry/codegraph sync
    npx -y @colbymchenry/codegraph status
    ```

## 6. Open Decisions

1. Should low-power software mode be default whenever software rendering is detected, or require explicit `WARP_SOFTWARE_LOW_POWER=1`?
2. Minimum acceptable spinner FPS: 4, 6, or 8? Current default plan/code uses 6 FPS.
3. Should CLI waiting dots become static in software mode, or animate slowly?
4. First pass Linux-only, or should Windows/mac software paths share same policy now?
5. Validation: is manual CPU measurement via `ps`/`pidstat` enough, or should an automated perf gate/script be required?

## 7. Gotchas

- Repo requires simplified Chinese comments/replies per `/mnt/Data/Apps/warp/AGENTS.md`, but user has caveman mode active; keep responses terse.
- User explicitly wants no VRAM usage. Do not propose GPU/VRAM fallback.
- TDD skill active: write failing test first, run RED, then implement GREEN, then refactor/commit.
- Git guardrails active: work on named branch, commit small logical chunks, never force push/reset hard/clean without ask.
- Current branch already named `feature/software-low-cpu-rendering`.
- `.codegraph/` is untracked and large; do not commit accidentally.
- `cargo fmt --check` showed unrelated formatting diffs in `/mnt/Data/Apps/warp/app/src/ai/agent_providers/chat_stream.rs`. Avoid formatting whole repo unless ready for unrelated changes; use `rustfmt <touched files>` for local formatting during atomic commits.
- `ctx_execute` runs from temp dir unless command explicitly `cd /mnt/Data/Apps/warp`; always include `cd` for cargo/git analysis commands via ctx tools.
- Pi NVIDIA model reliability:
  - Use `nvidia/qwen/qwen3-coder-480b-a35b-instruct` and `nvidia/mistralai/mistral-large-3-675b-instruct-2512` for subagents.
  - Avoid `nvidia/nvidia/llama-3.1-nemotron-ultra-253b-v1` (404) and `nvidia/deepseek-ai/deepseek-v4-pro` (timeout).
  - User authorized Gemini fallback if NVIDIA slow or failing.
- Pi subagent runs print nonblocking caveman extension stale-context error after answer:
  - `Extension error (...pi-caveman...): This extension ctx is stale after session replacement or reload...`
  - Treat as nonblocking if task output arrived.
- Terminal model lock caution from repo docs: avoid new `TerminalModel::lock()` calls in UI paths; use existing lock/try_lock flow and keep lock scope short.
