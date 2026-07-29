fn main() {
    if let Err(error) = rusty_procgen_preflight::run_cli() {
        eprintln!("rusty-procgen failed:");
        eprintln!("- {error}");
        std::process::exit(1);
    }
}
