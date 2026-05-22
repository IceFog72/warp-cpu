use settings::{macros::define_settings_group, SupportedPlatforms, SyncToCloud};
use std::time::Duration;
use warpui::platform::GraphicsBackend;

define_settings_group!(GPUSettings, settings: [
   force_software_rendering: ForceSoftwareRendering {
       type: bool,
       default: cfg!(any(target_os = "linux", target_os = "freebsd")),
       supported_platforms: SupportedPlatforms::LINUX,
       sync_to_cloud: SyncToCloud::Never,
       private: false,
       toml_path: "system.force_software_rendering",
       description: "Whether to force CPU software rendering on Linux.",
   },
   prefer_low_power_gpu: PreferLowPowerGPU {
       type: bool,
       // Opt for the low power (integrated) GPU on Windows / Linux since discrete GPUs tend to be
        // more unstable.
       default: cfg!(any(target_os = "linux", target_os = "freebsd", windows)),
       supported_platforms: SupportedPlatforms::ALL,
       sync_to_cloud: SyncToCloud::Never,
       private: false,
       toml_path: "system.prefer_low_power_gpu",
       description: "Whether to prefer the integrated (low-power) GPU.",
   },
   preferred_backend: PreferredGraphicsBackend {
       type: Option<GraphicsBackend>,
       default: None,
       supported_platforms: SupportedPlatforms::WINDOWS,
       sync_to_cloud: SyncToCloud::Never,
       private: false,
       toml_path: "system.preferred_graphics_backend",
       description: "The preferred graphics backend on Windows.",
   },
]);

pub const DEFAULT_SOFTWARE_TERMINAL_FPS: u64 = 6;

pub fn software_low_power_rendering_enabled() -> bool {
    software_low_power_rendering_enabled_from_env(
        std::env::var("WARP_SOFTWARE_LOW_POWER").ok().as_deref(),
        std::env::var("WARP_FORCE_SOFTWARE").ok().as_deref(),
        std::env::var("LIBGL_ALWAYS_SOFTWARE").ok().as_deref(),
    )
}

pub fn software_terminal_frame_interval() -> Duration {
    fps_interval(
        std::env::var("WARP_SOFTWARE_TERMINAL_FPS").ok().as_deref(),
        DEFAULT_SOFTWARE_TERMINAL_FPS,
    )
}

fn software_low_power_rendering_enabled_from_env(
    low_power: Option<&str>,
    force_software: Option<&str>,
    libgl_software: Option<&str>,
) -> bool {
    env_flag_enabled(low_power)
        || env_flag_enabled(force_software)
        || env_flag_enabled(libgl_software)
}

fn fps_interval(value: Option<&str>, default_fps: u64) -> Duration {
    let fps = value
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|fps| *fps > 0)
        .unwrap_or(default_fps);

    Duration::from_micros(1_000_000 / fps)
}

fn env_flag_enabled(value: Option<&str>) -> bool {
    matches!(value, Some("1" | "true" | "TRUE" | "True"))
}

#[cfg(test)]
#[path = "gpu_tests.rs"]
mod tests;
