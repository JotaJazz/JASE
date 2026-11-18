fn main() -> iced::Result {
    let osrelease = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    let wsl_distro = std::env::var("WSL_DISTRO_NAME").ok();
    if jase::startup::force_x11(&osrelease, wsl_distro.as_deref()) {
        // SAFETY: todavía no hay otros hilos que lean el entorno.
        unsafe { std::env::remove_var("WAYLAND_DISPLAY") }
    }
    if std::env::args().skip(1).any(|a| a == "--smoke") {
        jase::smoke::run();
    }
    jase::run()
}
