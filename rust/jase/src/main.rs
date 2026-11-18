fn main() -> iced::Result {
    let osrelease = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    let wsl_distro = std::env::var("WSL_DISTRO_NAME").ok();
    if jase::startup::force_x11(&osrelease, wsl_distro.as_deref()) {
        // SAFETY: todavía no hay otros hilos que lean el entorno.
        unsafe { std::env::remove_var("WAYLAND_DISPLAY") }
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--smoke") {
        jase::smoke::run();
    }
    if let Some(i) = args.iter().position(|a| a == "--screenshot") {
        let Some(dir) = args.get(i + 1) else {
            eprintln!("--screenshot <carpeta>");
            std::process::exit(2)
        };
        jase::screenshot::run(std::path::Path::new(dir));
    }
    jase::run()
}
