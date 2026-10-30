fn main() -> std::process::ExitCode {
    demerzel_lib::backend::cli::run(&std::env::args().skip(1).collect::<Vec<_>>())
}
