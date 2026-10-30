// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        demerzel_lib::run();
    } else {
        let result = demerzel_lib::backend::cli::run(&args);
        if result != std::process::ExitCode::SUCCESS { std::process::exit(1); }
    }
}
