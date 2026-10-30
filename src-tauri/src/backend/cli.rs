//! CLI `dem` / `demerzel`: status, up, down, sync, watch (#40).
//!
//! `dem sync` вызывает `commands.migrate` из `orchestrator.json` (#43); без поля — пропуск.
//! Per-user Unix socket supervisor owns child handles across CLI invocations.
use super::{config, docker, ecosystem, process::Status, registry::Registry};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const USAGE: &str = "usage: dem <status|up|down|sync|watch> [ROOT]\nReads demerzel.yaml or orchestrator.json in ROOT (default: current directory).\nwatch refreshes logs and CPU/RSS every second; Ctrl-C exits the viewer.";

const ACTIONS: &[&str] = &["up", "down", "sync", "status", "watch"];

#[derive(Serialize, Deserialize)]
struct Request { action: String, root: PathBuf }
#[derive(Serialize, Deserialize)]
struct Response { output: String, error: Option<String> }

fn runtime_dir() -> Result<PathBuf, String> {
    let dir = if let Some(override_dir) = std::env::var_os("DEMERZEL_RUNTIME_DIR") {
        PathBuf::from(override_dir)
    } else {
        let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
        PathBuf::from(home).join(".orchestrator/runtime")
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn connect(start: bool) -> Result<UnixStream, String> {
    let socket = runtime_dir()?.join("dem.sock");
    if let Ok(stream) = UnixStream::connect(&socket) { return Ok(stream); }
    if !start { return Err("supervisor is not running; run dem up first".into()); }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let log = std::fs::OpenOptions::new().create(true).append(true).open(runtime_dir()?.join("supervisor.log")).map_err(|e| e.to_string())?;
    use std::os::unix::process::CommandExt;
    let mut child = Command::new(exe).arg("__serve").stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|e| e.to_string())?).stderr(log).process_group(0)
        .spawn().map_err(|e| e.to_string())?;
    // Reap if the daemon exits while this invocation remains alive.
    std::thread::spawn(move || { let _ = child.wait(); });
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Ok(stream) = UnixStream::connect(&socket) { return Ok(stream); }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("supervisor did not start; see ~/.orchestrator/runtime/supervisor.log".into())
}

fn request(action: &str, root: &Path) -> Result<String, String> {
    // status/watch also start the supervisor so queries work without a prior `dem up`.
    let mut stream = connect(matches!(action, "up" | "sync" | "status" | "watch"))?;
    stream.set_read_timeout(Some(Duration::from_secs(180))).map_err(|e| e.to_string())?;
    let req = Request { action: action.into(), root: root.canonicalize().map_err(|e| e.to_string())? };
    let text = serde_json::to_string(&req).map_err(|e| e.to_string())?;
    writeln!(stream, "{text}").map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).map_err(|e| e.to_string())?;
    let response: Response = serde_json::from_str(&line).map_err(|e| e.to_string())?;
    match response.error { Some(e) => Err(e), None => Ok(response.output) }
}

fn format_migrate_status(name: &str, log: &str) -> String {
    format!("{name} migrate: {log}")
}

fn format_migrate_watch_line(name: &str, log: &str) -> String {
    format!("[{name} migrate] {log}")
}

fn run_command(command: &str, path: &Path, env: &BTreeMap<String, String>) -> Result<String, String> {
    let mut process = super::process::Managed::spawn(command, path, env, None).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let status = process.poll().map_err(|e| e.to_string())?;
        if status != Status::Running {
            // Reader threads can still be draining the final pipe data.
            std::thread::sleep(Duration::from_millis(30));
            let logs = process.logs().iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n");
            return if status == Status::Exited(0) { Ok(logs) } else { Err(format!("command failed ({status:?})\n{logs}")) };
        }
        if Instant::now() >= deadline {
            process.stop(Duration::from_secs(1)).map_err(|e| e.to_string())?;
            return Err("command timed out after 120 seconds".into());
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}

#[derive(Default)]
struct Supervisor {
    registry: Registry,
    ids: BTreeMap<(PathBuf, String), u64>,
    manifests: BTreeMap<PathBuf, ecosystem::Manifest>,
    migration_logs: BTreeMap<(PathBuf, String), String>,
}

impl Supervisor {
    fn execute(&mut self, req: Request) -> Result<String, String> {
        if !["up", "down", "sync", "status", "watch"].contains(&req.action.as_str()) { return Err("unknown command".into()); }
        // down uses the manifest that was started, even if the file is later edited.
        let manifest = if req.action == "down" {
            self.manifests.get(&req.root).cloned().unwrap_or(ecosystem::load(&req.root)?)
        } else { ecosystem::load(&req.root)? };
        let mut output = Vec::new();
        let mut errors = Vec::new();
        let rows = self.registry.list().map_err(|e| e.to_string())?;
        for (name, svc) in &manifest.services {
            let key = (req.root.clone(), name.clone());
            let result = (|| -> Result<String, String> {
                if svc.kind.as_deref() == Some("docker-compose") {
                    if req.action == "up" || req.action == "down" { docker::command(svc, &req.action)?; }
                    let mut containers = docker::parse_status(&docker::command(svc, "status")?)?;
                    if req.action == "sync" && !docker::containers_ready(&containers) {
                        docker::command(svc, "up")?;
                        containers = docker::parse_status(&docker::command(svc, "status")?)?;
                    }
                    if matches!(req.action.as_str(), "up" | "sync") {
                        if let Some(check) = &svc.healthcheck {
                            run_command(check, &svc.path, &BTreeMap::new())?;
                        }
                        docker::require_ready(&containers)?;
                    }
                    return Ok(format!("{name}: {}", serde_json::to_string(&containers).map_err(|e| e.to_string())?));
                }
                let id = self.ids.get(&key).copied();
                let row = id.and_then(|id| rows.iter().find(|r| r.id == id));
                let active = row.is_some_and(|r| r.status == Status::Running || r.status == Status::Starting);
                if req.action == "down" {
                    if let Some(id) = id { self.registry.stop(id, Duration::from_secs(3)).map_err(|e| e.to_string())?; }
                    return Ok(format!("{name}: stopped"));
                }
                let cfg = config::load_checked(&svc.path)?;
                let env = cfg.as_ref().map(|c| c.env.clone()).unwrap_or_default();
                if req.action == "sync" {
                    // ORM-agnostic: only `commands.migrate` from orchestrator.json (#43).
                    let log = match cfg
                        .as_ref()
                        .and_then(|c| config::resolve_migrate_command(&c.commands))
                    {
                        Some(migrate) => run_command(migrate, &svc.path, &env)?,
                        None => config::MIGRATE_SKIPPED.into(),
                    };
                    self.migration_logs.insert(key.clone(), log.clone());
                    output.push(format_migrate_status(name, &log));
                }
                if matches!(req.action.as_str(), "up" | "sync") && !active {
                    let cmd = svc.cmd.clone().or_else(|| config::resolve_dev_command(&svc.path)).ok_or_else(|| format!("{name}: no cmd/commands.dev/scripts.dev"))?;
                    let id = self.registry.start_service(name, &cmd, &svc.path, svc.port)?;
                    self.ids.insert(key.clone(), id);
                    if let Some(check) = &svc.healthcheck { run_command(check, &svc.path, &env)?; }
                    return Ok(format!("{name}: started #{id}"));
                }
                let state = row.map(|r| format!("{:?} pid={:?} port={:?}", r.status, r.pid, r.port)).unwrap_or_else(|| "stopped".into());
                let mut text = format!("{name}: {state}");
                if req.action == "watch" {
                    if let Some(id) = id {
                        for line in self.registry.logs(id).unwrap_or_default() { text.push_str(&format!("\n[{name}] {}", line.text)); }
                    }
                    if let Some(log) = self.migration_logs.get(&key) {
                        text.push_str(&format!("\n{}", format_migrate_watch_line(name, log)));
                    }
                    if let Some(pid) = row.and_then(|r| r.pid) {
                        if let Ok(metrics) = Command::new("ps").args(["-p", &pid.to_string(), "-o", "%cpu=,rss="]).output() {
                            text.push_str(&format!("\n[{name}] CPU% RSS(KiB): {}", String::from_utf8_lossy(&metrics.stdout).trim()));
                        }
                    }
                }
                Ok(text)
            })();
            match result { Ok(line) => output.push(line), Err(e) => errors.push(format!("{name}: {e}")) }
        }
        self.manifests.insert(req.root, manifest);
        if errors.is_empty() { Ok(output.join("\n")) } else { Err(format!("{}\n{}", output.join("\n"), errors.join("\n"))) }
    }
}

fn serve() -> Result<(), String> {
    let dir = runtime_dir()?;
    // An OS advisory lock is released even after a crash. It prevents a second
    // daemon from unlinking the live daemon's socket during concurrent `up`.
    let lock = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(dir.join("supervisor.lock")).map_err(|e| e.to_string())?;
    lock.try_lock().map_err(|e| format!("supervisor already running: {e}"))?;
    let socket = dir.join("dem.sock");
    match std::fs::remove_file(&socket) { Ok(()) => (), Err(e) if e.kind() == std::io::ErrorKind::NotFound => (), Err(e) => return Err(e.to_string()) }
    let listener = UnixListener::bind(&socket).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    let mut state = Supervisor::default();
    for stream in listener.incoming() {
        let mut stream = stream.map_err(|e| e.to_string())?;
        stream.set_read_timeout(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;
        let mut line = String::new();
        let response = match BufReader::new(&stream).read_line(&mut line) {
            Ok(_) if line.len() <= 65536 => serde_json::from_str::<Request>(&line).map_err(|e| e.to_string()).and_then(|req| state.execute(req)),
            Ok(_) => Err("request too large".into()),
            Err(e) => Err(e.to_string()),
        };
        let response = match response { Ok(output) => Response { output, error: None }, Err(error) => Response { output: String::new(), error: Some(error) } };
        let _ = writeln!(stream, "{}", serde_json::to_string(&response).map_err(|e| e.to_string())?);
    }
    Ok(())
}

pub fn run(args: &[String]) -> std::process::ExitCode {
    let action = args.first().map(String::as_str).unwrap_or("help");
    if ["help", "--help", "-h"].contains(&action) {
        println!("{USAGE}");
        return std::process::ExitCode::SUCCESS;
    }
    let result = if action == "__serve" { serve() } else {
        let root = args.get(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        if args.len() > 2 || !ACTIONS.contains(&action) {
            Err(USAGE.lines().next().unwrap_or(USAGE).into())
        } else if action == "watch" {
            loop {
                match request("watch", &root) {
                    Ok(text) => print!("\x1b[2J\x1b[H{text}\n"),
                    Err(e) => break Err(e),
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        } else {
            request(action, &root).map(|text| println!("{text}"))
        }
    };
    match result { Ok(()) => std::process::ExitCode::SUCCESS, Err(e) => { eprintln!("dem: {e}"); std::process::ExitCode::FAILURE } }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::ExitCode;

    fn code_u8(code: ExitCode) -> u8 {
        // ExitCode has no public as_u8; SUCCESS/FAILURE are the only values we assert.
        if code == ExitCode::SUCCESS {
            0
        } else if code == ExitCode::FAILURE {
            1
        } else {
            255
        }
    }

    #[test]
    fn help_flags_succeed() {
        for args in [vec![], vec!["help".into()], vec!["-h".into()], vec!["--help".into()]] {
            assert_eq!(code_u8(run(&args)), 0, "args={args:?}");
        }
    }

    #[test]
    fn unknown_and_extra_args_fail() {
        assert_eq!(code_u8(run(&["nope".into()])), 1);
        assert_eq!(code_u8(run(&["status".into(), ".".into(), "extra".into()])), 1);
    }

    #[test]
    fn actions_list_matches_issue_40() {
        for cmd in ["status", "up", "down", "sync", "watch"] {
            assert!(ACTIONS.contains(&cmd), "{cmd}");
        }
    }

    #[test]
    fn sync_migrate_status_and_watch_lines() {
        let skipped = format_migrate_status("api", config::MIGRATE_SKIPPED);
        assert_eq!(skipped, "api migrate: no commands.migrate; skipped");
        let watch = format_migrate_watch_line("api", "Applied 2 migrations");
        assert_eq!(watch, "[api migrate] Applied 2 migrations");
    }

    #[test]
    fn sync_uses_resolve_migrate_command_contract() {
        let with = config::Commands {
            migrate: Some("bun run db:migrate".into()),
            ..Default::default()
        };
        assert_eq!(
            config::resolve_migrate_command(&with),
            Some("bun run db:migrate")
        );
        assert!(config::resolve_migrate_command(&config::Commands::default()).is_none());
    }
}
