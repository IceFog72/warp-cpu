use super::*;

#[test]
fn software_low_power_rendering_is_enabled_by_any_software_env_flag() {
    assert!(software_low_power_rendering_enabled_from_env(
        Some("1"),
        None,
        None
    ));
    assert!(software_low_power_rendering_enabled_from_env(
        None,
        Some("true"),
        None
    ));
    assert!(software_low_power_rendering_enabled_from_env(
        None,
        None,
        Some("TRUE")
    ));
}

#[test]
fn software_low_power_rendering_ignores_disabled_env_flags() {
    assert!(!software_low_power_rendering_enabled_from_env(
        Some("0"),
        Some("false"),
        None
    ));
}

#[test]
fn software_terminal_frame_interval_uses_env_fps() {
    assert_eq!(
        fps_interval(Some("10"), DEFAULT_SOFTWARE_TERMINAL_FPS),
        Duration::from_millis(100)
    );
}

#[test]
fn software_terminal_frame_interval_uses_default_for_invalid_fps() {
    assert_eq!(
        fps_interval(Some("0"), DEFAULT_SOFTWARE_TERMINAL_FPS),
        Duration::from_micros(1_000_000 / DEFAULT_SOFTWARE_TERMINAL_FPS)
    );
}
