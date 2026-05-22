# Software Rendering Low-CPU Plan

Goal: reduce CPU use when Warp runs with forced software rendering and terminal output only animates small CLI progress/dots. Scope targets roughly 7/10 complexity: adaptive frame pacing, terminal wakeup batching, spinner heuristics, software-mode animation throttles, and measurement hooks. Excludes full renderer rewrite and VRAM usage.

## Design summary

- Keep wgpu/llvmpipe software rendering path.
- Stop rendering every terminal model generation when only tiny ephemeral output changes.
- Add software low-power render policy with burst/interactive/steady/idle phases.
- Batch terminal wakeups and drop intermediate spinner frames while keeping latest terminal state.
- Add telemetry/log counters to prove fewer redraws and lower CPU.
- Keep escape hatches through env vars and settings.

## Implementation tasks

### Task 1: Add software low-power constants and env parsing helper
**File(s)**: `crates/warpui/src/windowing/winit/event_loop/mod.rs`
**Action**: Add helper near `compute_min_frame_interval()`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SoftwareFrameClass {
    Idle,
    TerminalSteady,
    Interactive,
    Burst,
}

fn env_fps(name: &str) -> Option<u64> {
    std::env::var(name).ok()?.parse::<u64>().ok().filter(|fps| *fps > 0)
}

fn frame_interval_for_fps(fps: u64) -> Duration {
    Duration::from_millis((1000 / fps).max(1))
}
```

Default FPS values:
- `WARP_SOFTWARE_IDLE_FPS=0`
- `WARP_SOFTWARE_TERMINAL_FPS=6`
- `WARP_SOFTWARE_INTERACTIVE_FPS=12`
- `WARP_SOFTWARE_BURST_FPS=30`

**Verify**: `cargo fmt --check` succeeds or reports only pre-existing formatting outside touched file.

### Task 2: Replace single `min_frame_interval` with frame policy fields
**File(s)**: `crates/warpui/src/windowing/winit/event_loop/mod.rs`
**Action**: In `EventLoop`, replace or extend `min_frame_interval: Option<Duration>` with:

```rust
software_rendering: bool,
software_idle_interval: Option<Duration>,
software_terminal_interval: Duration,
software_interactive_interval: Duration,
software_burst_interval: Duration,
current_frame_class: SoftwareFrameClass,
burst_until: Option<Instant>,
last_frame_time: Option<Instant>,
```

Initialize from existing software detection logic in `compute_min_frame_interval()` without changing current env support for `WARP_SOFTWARE_FPS`.

**Verify**: `cargo check -p warpui --lib` reaches compile phase for this file; no missing field errors in `EventLoop` init.

### Task 3: Preserve existing `WARP_SOFTWARE_FPS` behavior as compatibility path
**File(s)**: `crates/warpui/src/windowing/winit/event_loop/mod.rs`
**Action**: If `WARP_SOFTWARE_FPS` is set, use it for terminal/interactive/burst intervals unless more specific env vars are set. Keep hardware path unlimited.

Sketch:

```rust
let legacy_fps = env_fps("WARP_SOFTWARE_FPS");
let terminal_fps = env_fps("WARP_SOFTWARE_TERMINAL_FPS").or(legacy_fps).unwrap_or(6);
let interactive_fps = env_fps("WARP_SOFTWARE_INTERACTIVE_FPS").or(legacy_fps).unwrap_or(12);
let burst_fps = env_fps("WARP_SOFTWARE_BURST_FPS").or(legacy_fps).unwrap_or(30);
```

**Verify**: `WARP_SOFTWARE_FPS=10 cargo check -p warpui --lib` compiles same code path.

### Task 4: Add frame-class selection entry points
**File(s)**: `crates/warpui/src/windowing/winit/event_loop/mod.rs`
**Action**: Add methods:

```rust
fn set_frame_class(&mut self, class: SoftwareFrameClass) { ... }
fn start_burst(&mut self, duration: Duration) { ... }
fn current_frame_interval(&self) -> Option<Duration> { ... }
```

Rules:
- `Burst` while `Instant::now() < burst_until`.
- `Idle` returns `software_idle_interval`, default `None` means do not self-trigger render.
- Hardware rendering returns `None`.

**Verify**: Unit-test helper logic if file has test module; otherwise `cargo check -p warpui --lib`.

### Task 5: Classify interactive window events as burst or interactive
**File(s)**: `crates/warpui/src/windowing/winit/event_loop/mod.rs`
**Action**: In `WindowEvent` handling:
- resize/scale-factor change/focus change/mouse wheel => `start_burst(Duration::from_millis(300))`
- keyboard input/text input/mouse click => `set_frame_class(SoftwareFrameClass::Interactive)` plus short burst if needed
- ordinary redraw from terminal stays terminal steady unless marked by app later.

**Verify**: Add temporary `log::trace!` for class changes behind existing logging style; run `cargo check -p warpui --lib`.

### Task 6: Apply adaptive interval before rendering
**File(s)**: `crates/warpui/src/windowing/winit/event_loop/mod.rs`
**Action**: Replace current fixed limiter around `RedrawRequested` with `current_frame_interval()`. Sleep only when a redraw is already requested and last frame is too recent.

Sketch:

```rust
if let Some(interval) = self.current_frame_interval() {
    if let Some(last) = self.last_frame_time {
        let elapsed = now.saturating_duration_since(last);
        if elapsed < interval {
            std::thread::sleep(interval - elapsed);
        }
    }
}
self.last_frame_time = Some(Instant::now());
```

**Verify**: Run `WARP_FORCE_SOFTWARE=1 WARP_SOFTWARE_TERMINAL_FPS=6 cargo check -p warpui --lib`.

### Task 7: Add terminal model output-shape tracking
**File(s)**: `app/src/terminal/model/terminal_model.rs`
**Action**: Add lightweight fields to `TerminalModel`:

```rust
last_output_shape: TerminalOutputShape,
last_output_shape_generation: u64,
```

Add enum:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalOutputShape {
    Unknown,
    Bulk,
    EphemeralSameLine,
}
```

Initialize to `Unknown`. Do not lock longer than current model update paths.

**Verify**: `cargo check -p warp --lib` reports no visibility/import errors for enum.

### Task 8: Detect ephemeral spinner-style output
**File(s)**: `app/src/terminal/model/terminal_model.rs`
**Action**: In PTY/grid update path near existing `increment_generation()` calls, classify small updates containing carriage return/backspace or same-row overwrite as `EphemeralSameLine`; classify large appended output/newline-heavy output as `Bulk`.

Heuristic:
- bytes <= 64 and contains `\r` or `\x08` => `EphemeralSameLine`
- changed row count <= 1 and no scrollback growth => `EphemeralSameLine`
- otherwise `Bulk`

If exact changed-row data unavailable, start with byte heuristic only.

**Verify**: Add unit test in `app/src/terminal/model/terminal_model_tests.rs` if existing harness supports direct helper test; otherwise isolate helper function and test it.

### Task 9: Expose output shape and generation without blocking UI
**File(s)**: `app/src/terminal/model/terminal_model.rs`
**Action**: Add getters:

```rust
pub fn output_shape(&self) -> TerminalOutputShape { self.last_output_shape }
pub fn output_shape_generation(&self) -> u64 { self.last_output_shape_generation }
```

Keep existing atomic `generation()` untouched.

**Verify**: `cargo check -p warp --lib`.

### Task 10: Add terminal software render throttle state
**File(s)**: `app/src/terminal/view.rs`
**Action**: Add fields to `TerminalView`:

```rust
last_software_terminal_notify: Option<Instant>,
pending_software_terminal_notify: bool,
```

Initialize in constructor where `last_rendered_generation` is initialized.

**Verify**: `cargo check -p warp --lib` finds all struct initializers updated.

### Task 11: Add helper to detect software low-power mode in terminal view
**File(s)**: `app/src/terminal/view.rs`, `app/src/settings/gpu.rs`
**Action**: Add helper in terminal view or settings:

```rust
fn software_low_power_terminal_enabled() -> bool {
    std::env::var("WARP_FORCE_SOFTWARE").is_ok()
        || std::env::var("LIBGL_ALWAYS_SOFTWARE").ok().as_deref() == Some("1")
        || std::env::var("WARP_SOFTWARE_LOW_POWER").ok().as_deref() == Some("1")
}
```

Prefer centralizing in `app/src/settings/gpu.rs` if existing GPU settings are already imported by `TerminalView`.

**Verify**: `cargo check -p warp --lib`.

### Task 12: Throttle terminal wakeup notifications for ephemeral output
**File(s)**: `app/src/terminal/view.rs`
**Action**: In `handle_terminal_wakeup`, after reading `current_generation`, inspect `model.output_shape()`. If software low-power mode and shape is `EphemeralSameLine`, only run full update/`ctx.notify()` every `WARP_SOFTWARE_TERMINAL_FPS` interval. Keep `last_rendered_generation` updated only when actually notifying.

Sketch:

```rust
let should_throttle = software_low_power_terminal_enabled()
    && model.output_shape() == TerminalOutputShape::EphemeralSameLine;
if should_throttle && !self.software_terminal_notify_due() {
    self.pending_software_terminal_notify = true;
    return;
}
```

**Verify**: Unit-test helper `software_terminal_notify_due()` with deterministic `Instant` wrapper if practical; otherwise manual log shows skipped wakeups under spinner.

### Task 13: Schedule delayed notify for pending terminal spinner frame
**File(s)**: `app/src/terminal/view.rs`
**Action**: When throttling skips a wakeup, schedule one future task/timer for the next allowed frame. Use existing async/timer style already used in file. Avoid spawning unbounded timers; guard with `pending_software_terminal_notify`.

Expected behavior: spinner keeps moving at 4–8 FPS, no event starvation.

**Verify**: Run a command like `while true; do printf '\r.'; sleep 0.02; done` in Warp; logs show at most configured terminal FPS notify calls.

### Task 14: Do not throttle bulk output, shell prompt, or completion events
**File(s)**: `app/src/terminal/view.rs`, `app/src/terminal/model/terminal_model.rs`
**Action**: Ensure output shape `Bulk` bypasses throttle. Ensure `ModelEvent::BlockCompleted`, `TerminalClear`, keypress/input edits, resize, and scroll call immediate `ctx.notify()` or burst path.

**Verify**: Run `yes | head -10000`, prompt return, typing, scrolling. Output should appear promptly and not feel stuck.

### Task 15: Add software animation policy helper
**File(s)**: `app/src/settings/gpu.rs`
**Action**: Add public helper:

```rust
pub fn software_low_power_animations_enabled() -> bool { ... }
pub fn software_animation_interval() -> Duration { ... }
```

Default interval 250ms (`4 FPS`). Env override: `WARP_SOFTWARE_ANIMATION_FPS`, disabled by `WARP_SOFTWARE_ANIMATIONS=0`.

**Verify**: `cargo check -p warp --lib`.

### Task 16: Throttle shimmering remote-server loading footer
**File(s)**: `app/src/terminal/view.rs`
**Action**: Locate `render_remote_server_loading_footer` and related `shimmering text animation` state. When software low-power animations enabled, either render static text or update at `software_animation_interval()`.

**Verify**: Remote server loading footer remains readable; no continuous 30/60 FPS animation in software mode.

### Task 17: Throttle CLI agent footer / dots animation
**File(s)**: `app/src/terminal/view.rs`, files referenced by `ambient_agent::render_loading_footer`, `use_agent_footer`
**Action**: Apply same software animation policy to terminal-visible loading dots. Prefer static text if animation framework cannot accept a custom interval.

**Verify**: Start CLI agent wait state; dots change no faster than 4 FPS or remain static if configured.

### Task 18: Add redraw counters under trace logging
**File(s)**: `crates/warpui/src/windowing/winit/event_loop/mod.rs`, `app/src/terminal/view.rs`
**Action**: Add trace logs/counters:
- rendered frame count per second in software mode
- skipped terminal wakeups per second
- output shape counts

Keep logs behind `log::trace!` or env-gated debug code.

**Verify**: `RUST_LOG=warpui=trace,warp=trace` shows counters without flooding more than once/sec.

### Task 19: Add docs for env tuning
**File(s)**: `README.md`
**Action**: Add “Software rendering low-power tuning” section documenting:
- `WARP_SOFTWARE_LOW_POWER=1`
- `WARP_SOFTWARE_TERMINAL_FPS=6`
- `WARP_SOFTWARE_INTERACTIVE_FPS=12`
- `WARP_SOFTWARE_BURST_FPS=30`
- `WARP_SOFTWARE_ANIMATION_FPS=4`
- `WARP_SOFTWARE_ANIMATIONS=0`

**Verify**: Markdown section renders and names match code.

### Task 20: Add focused terminal output-shape tests
**File(s)**: `app/src/terminal/model/terminal_model_tests.rs`
**Action**: Add tests for helper created in Task 8:
- `"\r.\r.."` => `EphemeralSameLine`
- `"loading\x08\x08"` => `EphemeralSameLine`
- 4KB output => `Bulk`
- newline-heavy output => `Bulk`

**Verify**: `cargo nextest run -p warp terminal_model --no-fail-fast` or nearest available test target.

### Task 21: Add frame policy unit tests
**File(s)**: `crates/warpui/src/windowing/winit/event_loop/mod.rs` or new `crates/warpui/src/windowing/winit/event_loop/tests.rs`
**Action**: Extract pure helper for env/default FPS policy. Test defaults and override precedence.

**Verify**: `cargo nextest run -p warpui software_frame_policy --no-fail-fast` or exact test name.

### Task 22: Manual performance script
**File(s)**: `script/measure-software-spinner-cpu.sh`
**Action**: Add script that:
- launches Warp with software env vars
- prints commands to run spinner workload
- samples process CPU with `pidstat` or `ps`
- writes results to `perf-data/software-spinner-YYYYMMDD.txt`

Do not require root or GPU.

**Verify**: Script runs on Linux and exits with instructions if `pidstat` missing.

### Task 23: Baseline measurement before code changes
**File(s)**: `perf-data/` output only, gitignored if already configured
**Action**: Run existing Warp before applying code changes. Capture CPU during spinner and idle.

**Verify**: Baseline file includes command, env vars, CPU %, date, commit hash.

### Task 24: Post-change measurement
**File(s)**: `perf-data/` output only
**Action**: Repeat same workload after implementation.

Target: under spinner, software-rendered Warp should render <= configured terminal FPS and CPU should drop materially versus baseline. Exact target depends machine; aim >50% reduction.

**Verify**: Compare baseline/post files; include summary in PR.

### Task 25: Run formatting and targeted checks
**File(s)**: touched Rust files
**Action**: Run:

```bash
cargo fmt
cargo check -p warpui --lib
cargo check -p warp --lib
```

Avoid full build unless needed.

**Verify**: Commands finish or failures are documented with exact errors.

### Task 26: Sync CodeGraph after edits
**File(s)**: `.codegraph/`
**Action**: Run:

```bash
npx -y @colbymchenry/codegraph sync
```

**Verify**: `npx -y @colbymchenry/codegraph status` says index up to date.

## Rollback

- If frame pacing breaks responsiveness, revert tasks 1–6 first; terminal batching can still stand alone behind `WARP_SOFTWARE_LOW_POWER=1`.
- If terminal output drops visible states incorrectly, disable Task 12/13 by making `software_low_power_terminal_enabled()` return false unless env explicitly set.
- If animation throttling causes UI stale states, keep only static animation disable path and revert interval-based custom scheduling.
- If tests become hard due internal APIs, keep pure helper tests and document missing integration test.

## Open questions

1. Should low-power software mode be default whenever software rendering is detected, or require `WARP_SOFTWARE_LOW_POWER=1`?
2. Minimum acceptable spinner FPS: 4, 6, or 8?
3. Should CLI waiting dots become static in software mode, or still animate slowly?
4. Is Linux only enough for first pass, or must Windows/mac software paths share policy?
5. Do we accept approximate CPU validation via `ps/pidstat`, or need automated benchmark gate?
