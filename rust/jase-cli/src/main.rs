fn main() {
    let argv: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let env = |name: &str| std::env::var(name).ok();
    let deps = jase_cli::Deps::real();
    let code = jase_cli::run(
        &argv,
        &env,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
        &deps,
    );
    std::process::exit(code);
}
