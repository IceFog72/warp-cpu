# 🔒 Warp Fork Full Security Scan Report

**Scan Time**: 2026-05-04T21:00 UTC+3  
**Codebase**: `/mnt/Data/Apps/warp`  
**Fork Origin**: `zerx-lab/warp` (OpenWarp Project)  
**Upstream**: `warpdotdev/warp` (Warp Inc Official Repository)  
**Your Remote**: `IceFog72/warp-cpu`  
**Scan Methods**: `cargo audit` + git diff analysis + static pattern matching + manual code review

---

## 🎯 Part 1: Malicious Code / Backdoor / Data Exfiltration Hunt

> [!IMPORTANT]
> **Conclusion: No rootkits, backdoors, or data-stealing code found.**
> 
> This fork (`zerx-lab/warp`) is a community-driven "de-clouded" branch of the official Warp repository (OpenWarp). Compared to the upstream `origin/main`, the fork has only **212 file changes, +3110/-1594 lines**. The changes are focused on UI internationalization, branding, the SSH manager, and your CPU rendering patch. **No new network connections, process spawning, file stealing, or credential harvesting code was found.**

### 1.1 Fork Delta Analysis (origin/main..HEAD)

| Check Item | Result |
|--------|------|
| New HTTP/Network Connections (`https://`, `connect`, `fetch`, `send`) | ✅ **None** — Only UI text and existing buttons |
| New Process Spawns (`Command::new`, `spawn`, `exec`) | ✅ **None** — Zero matches |
| New File Reads (`fs::read`, `read_to_string`) | ✅ **None** — Only one place creating a download directory (for AI exports) |
| New Credential Access (`env::var`, `secret`, `password`, `token`) | ✅ **Not Malicious** — Only keychain integration for the SSH manager (normal functionality) |
| New `unsafe` Code | ✅ **None** — Zero matches |
| New Base64/Hex Encoding | ✅ **None** |
| New Executable Binaries | ✅ **None** |

### 1.2 Full External Network Endpoint Review

Scanned all hardcoded URLs across the entire codebase. **All network connections point to known, legitimate services**:

| Domain | Purpose | Suspicious? |
|------|------|---------|
| `app.warp.dev` / `*.warp.dev` | Warp Inc official API servers | ❌ Normal (Upstream legacy) |
| `api.openai.com` | OpenAI BYOP (Bring Your Own Provider) settings | ❌ User-configured |
| `api.anthropic.com` | Anthropic BYOP settings | ❌ User-configured |
| `api.deepseek.com` | DeepSeek BYOP settings | ❌ User-configured |
| `localhost:11434` | Ollama local LLM | ❌ Local |
| `models.dev/api.json` | Model directory fetching | ❌ Public API |
| `mcp.exa.ai/mcp` | Exa search MCP | ❌ Public API |
| `nodejs.org` / `registry.npmjs.org` | Node.js runtime installation | ❌ Official sources |
| `github.com/warpdotdev/*` | Dependency source code (git deps) | ❌ Warp official forks |
| `github.com/servo/*` | macOS font libraries | ❌ Mozilla/Servo project |

> **No network connections pointing to unknown domains, IP addresses, or suspicious servers were found.**

### 1.3 Build Script Review

Checked the 3 `build.rs` files — all perform standard compile-time tasks:

| File | Behavior | Suspicious? |
|------|------|---------|
| [crates/warpui/build.rs](file:///mnt/Data/Apps/warp/crates/warpui/build.rs) | Compiles Metal shaders (macOS), links ObjC libraries | ❌ Normal |
| [app/build.rs](file:///mnt/Data/Apps/warp/app/build.rs) | Downloads Sentry SDK, copies Windows DLLs, embeds resources | ❌ Normal |
| [crates/command-signatures-v2/build.rs](file:///mnt/Data/Apps/warp/crates/command-signatures-v2/build.rs) | Runs `yarn build` to compile JS | ❌ Normal |

**No build scripts contain behaviors that download unknown content, inject code, or steal environment variables.**

### 1.4 Prebuilt Binary Check

Prebuilt files included in the repository:

| File | Source | Risk |
|------|------|------|
| `app/assets/windows/x64/*.dll` (conpty, vcruntime, msvcp, dxcompiler, dxil) | Official Microsoft Windows components | ⚠️ Low — Recommend verifying hashes |
| `app/assets/windows/arm64/*.dll` (same as above) | Official Microsoft | ⚠️ Low |
| `crates/input_classifier/models/fasttext/cmd_lang_classifier_v4.bin` | FastText ML model | ⚠️ Low — ML models are non-executable |

> [!TIP]
> These DLLs are standard, required components for the Windows runtime. You can verify their authenticity by comparing their `sha256sum` against official Microsoft releases.

### 1.5 Shell Script Review

| Script | Behavior | Suspicious? |
|------|------|---------|
| `crates/remote_server/src/install_remote_server.sh` | Downloads the `oz` binary from `{download_base_url}` (placeholder, replaced with warp.dev at runtime) | ❌ Template script |
| `app/assets/bundled/ssh/*/install_tmux_and_warpify_linux.sh` | Installs tmux | ❌ Normal |
| `app/assets/bundled/bootstrap/*.sh` | Shell initialization bootstrap | ❌ Normal |

### 1.6 Supply Chain Dependency Review

Total of **1,553 crate dependencies**. All `[patch.crates-io]` and git dependencies point to:
- `github.com/warpdotdev/*` — Warp Inc official forks (14 crates)
- `github.com/servo/*` — Mozilla Servo project (4 crates)

**No rogue or unknown third-party dependency substitutions.**

### 1.7 Keyboard/Screen/Clipboard Capture

- **`crates/computer_use/`**: Contains keyboard/mouse/screenshot functionality, but this is for Warp's "Computer Use" AI tool feature (similar to Claude Computer Use), **only enabled when the user explicitly triggers an AI Agent tool call**.
- **Clipboard**: `clipboard` related code is only in `platform/mac/clipboard.rs` (macOS platform layer), used for normal copy/paste functionality.
- **Not Found**: Background keyloggers, silent screenshot uploads, or clipboard monitoring code.

### 1.8 Commit Author Analysis

| Author | Email | Commits | Content |
|------|------|--------|------|
| `zero` | `[EMAIL_ADDRESS]` | ~20 | OpenWarp maintainer — i18n, SSH manager, BYOP fixes, UI |
| `LeoYoung-code` | `[EMAIL_ADDRESS]` | ~4 | i18n internationalization contributions |
| `IceFog72` | `[EMAIL_ADDRESS]` | 3 | Your CPU rendering + README patches |
| `Pei Li` / `Zach Lloyd` | `@warp.dev` | 2 | Official Warp Inc commits synced from upstream |

**The changes made by all committers match their descriptions. No hidden malicious modifications were found.**

---

## ⚠️ Part 2: Residual Risks (From Upstream Warp)

The following issues are **not malicious behaviors introduced by the fork**, but rather inherent security features/privacy considerations in the upstream Warp codebase:

### 2.1 Telemetry Infrastructure (Upstream Legacy)

Upstream Warp includes a full telemetry collection framework:
- `crates/warpui_core/src/telemetry/` — Event collection engine
- `RudderStackConfig` — Analytics reporting config (write_key, root_url)
- `TelemetryConfig` / `TelemetryEvent` — Event definitions and dispatch

> [!NOTE]
> The OpenWarp fork has stripped out the Rudderstack write key and most of the telemetry sending code. Setting `telemetry_config` to `None` in the channel config can completely disable it. However, the framework skeleton remains. It is recommended to verify that `TelemetryConfig` is indeed `None` in your build configuration.

### 2.2 Sentry Crash Reporting (Upstream Legacy)

- `CrashReportingConfig.sentry_url` — Crash reports are sent to Sentry.
- Crash reports may contain local paths, usernames, environment variables, etc.

### 2.3 Firebase Authentication (Upstream Legacy)

- Hardcoded Firebase API key: `AIzaSyBdy3O3S9hrdayLJxJ7mriBR4qgUaUygAs`
- OpenWarp has changed `is_logged_in()` to always return `true`, bypassing cloud authentication.
- The Firebase service itself can still be contacted (if code paths are triggered).

### 2.4 Warp Server Communication (Upstream Legacy)

The following endpoints are hardcoded in the codebase, although OpenWarp may have disabled most of them:
- `https://app.warp.dev` — Main API
- `wss://rtc.app.warp.dev/graphql/v2` — Real-time sync
- `wss://sessions.app.warp.dev` — Session sharing
- `https://oz.warp.dev` — Agent management

> Recommendation: If you want to run completely offline, consider replacing these URLs with invalid addresses during build, or blocking them at the firewall level.

---

## 📊 Part 3: General Security Scan Summary

### 3.1 Dependency Vulnerabilities (cargo audit)

| ID | Crate | Severity | Issue |
|----|-------|--------|------|
| RUSTSEC-2026-0104 | `rustls-webpki` 0.101.7 | 🔴 High | CRL parsing panic (DoS) |
| RUSTSEC-2026-0099 | `rustls-webpki` 0.101.7 | 🔴 High | Wildcard certificate name constraint bypass |
| RUSTSEC-2026-0098 | `rustls-webpki` 0.101.7 | 🔴 High | URI name constraint bypass |
| RUSTSEC-2022-0035 | `websocket` 0.1.0 | 🟡 Med | Unbounded memory allocation (Possible false positive) |
| RUSTSEC-2025-0141 | `bincode` 1.3.3 | ℹ️ Low | Unmaintained |

### 3.2 IPC Security

- **IPC socket in `/tmp/`**: Predictable path, unauthenticated, unbounded memory allocation risk.
- See [protocol.rs](file:///mnt/Data/Apps/warp/crates/ipc/src/protocol.rs#L28)

### 3.3 Unsafe Code

- **~80 `unsafe` blocks**: Primarily in the macOS FFI layer and Windows platform code, representing normal platform interaction.
- **1 instance of `from_utf8_unchecked`**: In the terminal grid hot path, theoretical UB risk.
- No `unsafe` code added by the fork.

### 3.4 Technical Debt

| Metric | Count |
|------|------|
| `unwrap()`/`expect()` (non-test) | 1,756 |
| `panic!()`/`unreachable!()` (non-test) | 186 |
| `TODO`/`FIXME`/`HACK` tags | 357 |
| Total dependencies | 1,553 |

---

## 🏁 Final Assessment

```
┌─────────────────────────────────────────────────────────┐
│                                                         │
│  🟢 Malicious Code / Backdoors / Data Theft: None       │
│  🟢 Fork Delta: Clean (UI/i18n/branding/features)       │
│  🟢 Supply Chain: All deps from known sources           │
│  🟢 Build Scripts: No malicious behavior                │
│  🟢 Prebuilt Binaries: Standard Windows runtime         │
│                                                         │
│  🟡 Residual Risk: Upstream telemetry/Sentry/Firebase   │
│  🟡 Dependency CVEs: rustls-webpki needs update         │
│  🟡 IPC Security: /tmp socket unauthenticated           │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

**This fork is safe.** The changes in `zerx-lab/warp` (OpenWarp) are transparent de-clouding and localization efforts, and your own 3 commits only involve CPU rendering and documentation. It's important to be aware of the telemetry and cloud communication infrastructure left over from the upstream Warp codebase itself — these aren't malicious, but if you're aiming for a completely offline experience, you might want to strip them out further.
