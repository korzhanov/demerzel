//! CLI `dem` (алиас `demerzel`), #40.
//!
//! Реализовано: `status`, `help`. Команды `up`, `down`, `sync`, `watch`
//! будут подключены к менеджеру процессов и Docker в следующих PR.

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use demerzel_lib::backend::config;
use demerzel_lib::backend::scanner::{self, Project};

const USAGE: &str = "usage: dem <command> [args]

commands:
  status [ROOT...]   показать проекты и их конфиги (по умолчанию текущая папка)
  up | down          запуск и остановка (ещё не подключено, см. #40)
  sync               перезапуск и миграции (ещё не подключено, см. #40)
  watch              живой мониторинг (ещё не подключено, см. #40)
  help               эта справка";

fn print_tree(projects: &[Project], depth: usize) {
    for p in projects {
        let indent = "  ".repeat(depth);
        let cfg_path = p.path.join(config::CONFIG_FILE);
        let detail = match std::fs::read_to_string(&cfg_path) {
            Ok(json) => match config::parse(&json) {
                Ok(cfg) => match cfg.ports {
                    Some(ports) => format!("{:?} ports {}-{}", cfg.kind, ports.range[0], ports.range[1]),
                    None => format!("{:?}", cfg.kind),
                },
                Err(e) => format!("config error: {e}"),
            },
            Err(_) => "no orchestrator.json".to_string(),
        };
        println!("{indent}{}  [{}]  {}", p.name, detail, p.path.display());
        print_tree(&p.children, depth + 1);
    }
}

fn status(roots: &[PathBuf]) -> ExitCode {
    let mut code = ExitCode::SUCCESS;
    for root in roots {
        match scanner::scan(root) {
            Ok(projects) if projects.is_empty() => println!("{}: проектов не найдено", root.display()),
            Ok(projects) => {
                println!("{}:", root.display());
                print_tree(&projects, 1);
            }
            Err(e) => {
                let msg = scanner::access_hint(&e, root).unwrap_or_else(|| format!("{}: {e}", root.display()));
                eprintln!("dem: {msg}");
                code = ExitCode::FAILURE;
            }
        }
    }
    code
}

fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        None | Some("help") | Some("-h") | Some("--help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("status") => {
            let roots: Vec<PathBuf> = if args.len() > 1 {
                args[1..].iter().map(PathBuf::from).collect()
            } else {
                vec![env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf())]
            };
            status(&roots)
        }
        Some(cmd @ ("up" | "down" | "sync" | "watch")) => {
            eprintln!("dem {cmd}: ещё не подключено к менеджеру процессов (см. #40)");
            ExitCode::from(2)
        }
        Some(other) => {
            eprintln!("dem: неизвестная команда '{other}'\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    run(&args)
}
