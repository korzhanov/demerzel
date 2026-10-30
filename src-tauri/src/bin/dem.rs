//! Бинарник CLI `dem` (#40). Пакетный бинарь `demerzel` — GUI/default-run.
//!
//! Точка входа делегирует в `demerzel_lib::backend::cli::run`.

fn main() -> std::process::ExitCode {
    demerzel_lib::backend::cli::run(&std::env::args().skip(1).collect::<Vec<_>>())
}
