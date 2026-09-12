# Handoff: softbuffer CPU renderer (OpenWarp Linux)

Date: 2026-09-12 ~13:00 UTC+3. Prior related: `.skills/handoffs/2026-05-22-05-42-software-rendering-low-cpu.md`.

## Goal and constraints
Ship a CPU raster present path (`Scene` → pixels → softbuffer, zero wgpu/llvmpipe)
for Linux software rendering that is correct AND fast. Constraints: Linux-only,
feature-gated (`warpui/cpu-renderer`), opt-in at runtime (`WARP_CPU_RENDERER=1`),
GPU path behavior byte-identical, minimal diffs, caveplain terse style. Unlazy
ledger in `GATES.md` (7/8 green; G7 manual perf probe open).

## Current state (worktree, base `a0ff3c1`, all uncommitted)
Modified: `Cargo.lock`, `app/Cargo.toml` (target-gated `warpui/cpu-renderer`),
`crates/warpui/Cargo.toml` (feature + optional softbuffer 0.4.8),
`crates/warpui/src/rendering/mod.rs`, `crates/warpui/src/windowing/winit/window.rs`.
Untracked: `crates/warpui/src/rendering/cpu/{mod,tests}.rs`, `scripts/check-cpu-*.mjs`
(8 oracles incl. new `check-cpu-perf.mjs`), `GATES.md`.
Review loop with external agent via `~/Downloads/cpu-review-fixed-v{1..5}.zip`
(v1 file is `cpu-review-fixed.zip`): v1 opaque-window+alpha+early-branch,
v2 red-channel glyph coverage, v3 stride-bytes normalization, v4 per-window glyph
cache, **v5 (current): same-scene present skip, shift/add alpha (no /255 div),
32.32 fixed-point image scaling, 2s telemetry (`cpu renderer perf:` lines)**.
Local deviation from reviewer code: `std::time::Instant` → `instant::Instant`
(repo `.clippy.toml` bans std Instant; their v5 failed G6 until fixed).
Binary `target/debug/warp-oss` freshly built WITH v5+fix (3m42s, exit 0) but
**never launched** — telemetry collection is the immediate next step.

## Evidence (PASS vs not-run)
- PASS: 20/20 `cargo test -p warpui --features cpu-renderer --lib rendering::cpu`.
- PASS: clippy zero-diagnostics in touched files (JSON-span scoped oracle;
  repo-wide `clippy -D warnings` red pre-existing on `warpui_core` type_complexity).
- PASS: new files rustfmt-clean (repo-wide fmt red pre-existing — don't gate on it).
- PASS: static oracles deps/rects/glyphs/branch/present/perf/build + `cargo check -p warpui`
  with and without feature.
- PASS (earlier runs): first-frame stats healthy (dark ~88%, 3–5k glyphs),
  `/tmp/cpufirst.ppm` numerically sane (balanced RGB, text-like rows).
- NOT-RUN: v5 runtime telemetry — no `cpu renderer perf:` lines collected yet.

## Decisions
- `WARP_CPU_RENDERER` opt-in env (not `WARP_FORCE_SOFTWARE`) so llvmpipe users unaffected.
- Spike policy in `render_cpu`: skip frame, never break redraw loop.
- Opaque native window for CPU only; XRGB `0x00RRGGBB` packing per softbuffer contract.
- PPM dump + first-frame stats stay (one-shot, reviewer wants the discriminator).

## Next action
1. Launch: `WARP_CPU_RENDERER=1 ./target/debug/warp-oss` (bg, log to /tmp).
   Use `--bin warp-oss`; the `warp` bin needs unbuildable `warp-channel-config`.
2. Keep window alive 5+ min past indexing; map via `/tmp/xmap2.py` if unmapped.
3. Pull several `cpu renderer perf:` lines from `~/.local/state/openwarp/openwarp.log`,
   send to reviewer → v6 direction (damage rects vs scheduling).
4. Then: record G7/G8 evidence via `gate-check.mjs --approve GATES.md`.

## Blockers/gotchas
- Never `pkill -f` with a pattern matching the launcher cmdline (self-killed a run once).
- Restored window sometimes starts Unmapped (hit once); `python3.12` + python-xlib
  helpers in `/tmp/xlist.py`, `/tmp/xmap2.py` (no xwininfo/wmctrl installed).
- `openwarp.log` rotates; only current run present. `392759` "warp" window is
  xfce4-terminal, not warp. This model can't view images — user is visual oracle.
