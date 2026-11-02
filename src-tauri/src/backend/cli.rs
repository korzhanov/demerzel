//! CLI `dem` / `demerzel`: status, up, down, sync, watch (#40); pulse / restart (#102).
//!
//! `dem sync` вызывает `commands.migrate` из `orchestrator.json` (#43); без поля — пропуск.
//! Per-user Unix socket supervisor owns child handles across CLI invocations.
//! `dem pulse` / `dem restart` ходят в тот же `dem.sock` (локальный API для App Intents).
use super::{
    config, docker, ecosystem,
    process::Status,
    pulse::{PulsePayload, PulseStore},
    registry::Registry, summary,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const USAGE: &str = "\
usage: dem <status|up|down|sync|watch> [ROOT]
       dem pulse [PROJECT] [--json] [ROOT]
       dem restart <PROJECT> [--json] [ROOT]
Reads demerzel.yaml or orchestrator.json in ROOT (default: current directory).
pulse polls project progress (#97); restart stops and starts one service.
watch refreshes logs and CPU/RSS every second; Ctrl-C exits the viewer.
Local API: Unix socket ~/.orchestrator/runtime/dem.sock (no network).";

const INFRA_ACTIONS: &[&str] = &["up", "down", "sync", "status", "watch"];
const PROJECT_ACTIONS: &[&str] = &["pulse", "restart"];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Request {
    action: String,
    root: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    project: Option<String>,
    #[serde(default)]
    json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Response {
    output: String,
    error: Option<String>,
}

/// JSON-ответ `dem pulse --json` / сокет `action=pulse`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PulseJson {
    project: String,
    #[serde(flatten)]
    pulse: PulsePayload,
}

/// JSON-ответ `dem restart --json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RestartJson {
    project: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u64>,
}

fn runtime_dir() -> Result<PathBuf, String> {
    let dir = if let Some(override_dir) = std::env::var_os("DEMERZEL_RUNTIME_DIR") {
        PathBuf::from(override_dir)
    } else {
        let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
        PathBuf::from(home).join(".orchestrator/runtime")
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())?;
    Ok(dir)
}

fn connect(start: bool) -> Result<UnixStream, String> {
    let socket = runtime_dir()?.join("dem.sock");
    if let Ok(stream) = UnixStream::connect(&socket) {
        return Ok(stream);
    }
    if !start {
        return Err("supervisor is not running; run dem up first".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(runtime_dir()?.join("supervisor.log"))
        .map_err(|e| e.to_string())?;
    use std::os::unix::process::CommandExt;
    let mut child = Command::new(exe)
        .arg("__serve")
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log)
        .process_group(0)
        .spawn()
        .map_err(|e| e.to_string())?;
    // Reap if the daemon exits while this invocation remains alive.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Ok(stream) = UnixStream::connect(&socket) {
            return Ok(stream);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("supervisor did not start; see ~/.orchestrator/runtime/supervisor.log".into())
}

fn request(req: &Request) -> Result<String, String> {
    // status/watch/pulse/restart also start the supervisor so queries work without a prior `dem up`.
    let starts = matches!(
        req.action.as_str(),
        "up" | "sync" | "status" | "watch" | "pulse" | "restart"
    );
    let mut stream = connect(starts)?;
    stream
        .set_read_timeout(Some(Duration::from_secs(180)))
        .map_err(|e| e.to_string())?;
    let mut req = req.clone();
    req.root = req.root.canonicalize().map_err(|e| e.to_string())?;
    let text = serde_json::to_string(&req).map_err(|e| e.to_string())?;
    writeln!(stream, "{text}").map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    let response: Response = serde_json::from_str(&line).map_err(|e| e.to_string())?;
    match response.error {
        Some(e) => Err(e),
        None => Ok(response.output),
    }
}

fn format_migrate_status(name: &str, log: &str) -> String {
    format!("{name} migrate: {log}")
}

fn format_migrate_watch_line(name: &str, log: &str) -> String {
    format!("[{name} migrate] {log}")
}

fn run_command(command: &str, path: &Path, env: &BTreeMap<String, String>) -> Result<String, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("demerzel-run-{}-{nanos}", std::process::id()));
    let logs = super::process::LogFiles { out: dir.join("out.log"), err: dir.join("err.log") };
    let mut process =
        super::process::Managed::spawn(command, path, env, None, &logs).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let status = process.poll().map_err(|e| e.to_string())?;
        if status != Status::Running {
            // Читатели файлов делают последний проход после завершения процесса.
            std::thread::sleep(super::logs::TAIL_INTERVAL + Duration::from_millis(50));
            let logs = process
                .logs()
                .iter()
                .map(|l| l.text.clone())
                .collect::<Vec<_>>()
                .join("\n");
            let _ = std::fs::remove_dir_all(&dir);
            return if status == Status::Exited(0) {
                Ok(logs)
            } else {
                Err(format!("command failed ({status:?})\n{logs}"))
            };
        }
        if Instant::now() >= deadline {
            process
                .stop(Duration::from_secs(1))
                .map_err(|e| e.to_string())?;
            return Err("command timed out after 120 seconds".into());
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}

/// Разбор `dem pulse|restart …`: флаги `--json`, имя проекта, ROOT.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedArgs {
    action: String,
    root: PathBuf,
    project: Option<String>,
    json: bool,
}

fn looks_like_root(arg: &str) -> bool {
    arg == "."
        || arg == ".."
        || arg.starts_with('/')
        || arg.starts_with("./")
        || arg.starts_with("../")
        || arg.starts_with('~')
        || Path::new(arg).is_dir()
}

fn parse_args(args: &[String]) -> Result<ParsedArgs, String> {
    let action = args
        .first()
        .map(String::as_str)
        .unwrap_or("help");
    if ["help", "--help", "-h"].contains(&action) {
        return Err("help".into());
    }
    if action == "__serve" {
        return Ok(ParsedArgs {
            action: action.into(),
            root: PathBuf::from("."),
            project: None,
            json: false,
        });
    }

    let mut json = false;
    let mut positionals = Vec::new();
    for arg in args.iter().skip(1) {
        if arg == "--json" {
            json = true;
            continue;
        }
        if arg.starts_with('-') {
            return Err(USAGE.lines().next().unwrap_or(USAGE).into());
        }
        positionals.push(arg.clone());
    }

    if INFRA_ACTIONS.contains(&action) {
        if positionals.len() > 1 {
            return Err(USAGE.lines().next().unwrap_or(USAGE).into());
        }
        if json {
            return Err("dem: --json is for pulse/restart".into());
        }
        let root = positionals
            .first()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        return Ok(ParsedArgs {
            action: action.into(),
            root,
            project: None,
            json: false,
        });
    }

    if !PROJECT_ACTIONS.contains(&action) {
        return Err(USAGE.lines().next().unwrap_or(USAGE).into());
    }

    let mut project = None;
    let mut root = PathBuf::from(".");
    match positionals.as_slice() {
        [] => {
            if action == "restart" {
                return Err("dem restart: PROJECT required".into());
            }
        }
        [only] => {
            if looks_like_root(only) {
                if action == "restart" {
                    return Err("dem restart: PROJECT required".into());
                }
                root = PathBuf::from(only);
            } else {
                project = Some(only.clone());
            }
        }
        [name, path] => {
            if looks_like_root(name) && !looks_like_root(path) {
                return Err(USAGE.lines().next().unwrap_or(USAGE).into());
            }
            project = Some(name.clone());
            root = PathBuf::from(path);
        }
        _ => return Err(USAGE.lines().next().unwrap_or(USAGE).into()),
    }

    if action == "restart" && project.as_ref().is_none_or(|s| s.trim().is_empty()) {
        return Err("dem restart: PROJECT required".into());
    }

    Ok(ParsedArgs {
        action: action.into(),
        root,
        project,
        json,
    })
}

#[derive(Default)]
struct Supervisor {
    registry: Registry,
    ids: BTreeMap<(PathBuf, String), u64>,
    manifests: BTreeMap<PathBuf, ecosystem::Manifest>,
    migration_logs: BTreeMap<(PathBuf, String), String>,
    pulse_store: PulseStore,
}

impl Supervisor {
    fn execute(&mut self, req: Request) -> Result<String, String> {
        match req.action.as_str() {
            "pulse" => self.execute_pulse(&req),
            "restart" => self.execute_restart(&req),
            action if INFRA_ACTIONS.contains(&action) => self.execute_infra(req),
            _ => Err("unknown command".into()),
        }
    }

    fn execute_infra(&mut self, req: Request) -> Result<String, String> {
        // down uses the manifest that was started, even if the file is later edited.
        let manifest = if req.action == "down" {
            self.manifests
                .get(&req.root)
                .cloned()
                .unwrap_or(ecosystem::load(&req.root)?)
        } else {
            ecosystem::load(&req.root)?
        };
        let mut output = Vec::new();
        let mut errors = Vec::new();
        let rows = self.registry.list().map_err(|e| e.to_string())?;
        for (name, svc) in &manifest.services {
            let key = (req.root.clone(), name.clone());
            let result = (|| -> Result<String, String> {
                if svc.kind.as_deref() == Some("docker-compose") {
                    if req.action == "up" || req.action == "down" {
                        docker::command(svc, &req.action)?;
                    }
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
                    return Ok(format!(
                        "{name}: {}",
                        serde_json::to_string(&containers).map_err(|e| e.to_string())?
                    ));
                }
                let id = self.ids.get(&key).copied();
                let row = id.and_then(|id| rows.iter().find(|r| r.id == id));
                let active =
                    row.is_some_and(|r| r.status == Status::Running || r.status == Status::Starting);
                if req.action == "down" {
                    if let Some(id) = id {
                        self.registry
                            .stop(id, Duration::from_secs(3))
                            .map_err(|e| e.to_string())?;
                    }
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
                    let cmd = svc
                        .cmd
                        .clone()
                        .or_else(|| config::resolve_dev_command(&svc.path))
                        .ok_or_else(|| {
                            format!("{name}: no cmd/commands.dev/scripts.dev")
                        })?;
                    let id = self.registry.start_service(name, &cmd, &svc.path, svc.port)?;
                    self.ids.insert(key.clone(), id);
                    if let Some(check) = &svc.healthcheck {
                        run_command(check, &svc.path, &env)?;
                    }
                    return Ok(format!("{name}: started #{id}"));
                }
                let state = row
                    .map(|r| format!("{:?} pid={:?} port={:?}", r.status, r.pid, r.port))
                    .unwrap_or_else(|| "stopped".into());
                let mut text = format!("{name}: {state}");
                if req.action == "watch" {
                    if let Some(id) = id {
                        for line in self.registry.logs(id).unwrap_or_default() {
                            text.push_str(&format!("\n[{name}] {}", line.text));
                        }
                    }
                    if let Some(log) = self.migration_logs.get(&key) {
                        text.push_str(&format!("\n{}", format_migrate_watch_line(name, log)));
                    }
                    if let Some(pid) = row.and_then(|r| r.pid)
                        && let Ok(metrics) = Command::new("ps")
                            .args(["-p", &pid.to_string(), "-o", "%cpu=,rss="])
                            .output()
                        {
                            text.push_str(&format!(
                                "\n[{name}] CPU% RSS(KiB): {}",
                                String::from_utf8_lossy(&metrics.stdout).trim()
                            ));
                        }
                }
                Ok(text)
            })();
            match result {
                Ok(line) => output.push(line),
                Err(e) => errors.push(format!("{name}: {e}")),
            }
        }
        self.manifests.insert(req.root, manifest);
        if errors.is_empty() {
            Ok(output.join("\n"))
        } else {
            Err(format!("{}\n{}", output.join("\n"), errors.join("\n")))
        }
    }

    fn resolve_services<'a>(
        &'a self,
        manifest: &'a ecosystem::Manifest,
        project: Option<&str>,
        require_pulse: bool,
    ) -> Result<Vec<(&'a str, &'a ecosystem::Service)>, String> {
        let names: Vec<_> = if let Some(name) = project.map(str::trim).filter(|s| !s.is_empty()) {
            if !manifest.services.contains_key(name) {
                let known: Vec<_> = manifest.services.keys().cloned().collect();
                return Err(format!(
                    "unknown project {name}; known: {}",
                    known.join(", ")
                ));
            }
            vec![name.to_string()]
        } else if require_pulse {
            let with_pulse: Vec<String> = manifest
                .services
                .iter()
                .filter(|(_, svc)| {
                    config::load_checked(&svc.path)
                        .ok()
                        .flatten()
                        .and_then(|c| c.pulse)
                        .is_some()
                })
                .map(|(n, _)| n.clone())
                .collect();
            if with_pulse.is_empty() {
                return Err(
                    "no project with pulse config; pass PROJECT or set pulse in orchestrator.json"
                        .into(),
                );
            }
            with_pulse
        } else {
            return Err("PROJECT required".into());
        };
        let mut out = Vec::with_capacity(names.len());
        for n in names {
            let (key, svc) = manifest
                .services
                .get_key_value(&n)
                .ok_or_else(|| format!("unknown project {n}"))?;
            out.push((key.as_str(), svc));
        }
        Ok(out)
    }

    fn execute_pulse(&mut self, req: &Request) -> Result<String, String> {
        let manifest = ecosystem::load(&req.root)?;
        let services = self.resolve_services(&manifest, req.project.as_deref(), true)?;
        let mut rows_json = Vec::new();
        let mut rows_text = Vec::new();
        for (name, svc) in services {
            let cfg = config::load_checked(&svc.path)?.ok_or_else(|| {
                format!("{name}: orchestrator.json required for pulse")
            })?;
            let pulse = cfg
                .pulse
                .as_ref()
                .ok_or_else(|| format!("{name}: no pulse in orchestrator.json"))?;
            let store_key = format!("{}::{name}", req.root.display());
            let payload = self
                .pulse_store
                .poll(&store_key, &svc.path, pulse, &cfg.env)
                .map_err(|e| format!("{name}: {e}"))?;
            if req.json {
                rows_json.push(PulseJson {
                    project: name.to_string(),
                    pulse: payload,
                });
            } else {
                let snap = payload.to_project_pulse();
                rows_text.push(summary::format_pulse_line(name, &snap));
            }
        }
        self.manifests.insert(req.root.clone(), manifest);
        if req.json {
            if rows_json.len() == 1 {
                Ok(serde_json::to_string_pretty(&rows_json[0]).map_err(|e| e.to_string())?)
            } else {
                Ok(serde_json::to_string_pretty(&rows_json).map_err(|e| e.to_string())?)
            }
        } else {
            Ok(rows_text.join("\n"))
        }
    }

    fn execute_restart(&mut self, req: &Request) -> Result<String, String> {
        let name = req
            .project
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or("dem restart: PROJECT required")?;
        let manifest = ecosystem::load(&req.root)?;
        let svc = manifest
            .services
            .get(name)
            .ok_or_else(|| {
                let known: Vec<_> = manifest.services.keys().cloned().collect();
                format!("unknown project {name}; known: {}", known.join(", "))
            })?
            .clone();
        let key = (req.root.clone(), name.to_string());

        if svc.kind.as_deref() == Some("docker-compose") {
            docker::command(&svc, "restart")?;
            let containers = docker::parse_status(&docker::command(&svc, "status")?)?;
            if let Some(check) = &svc.healthcheck {
                run_command(check, &svc.path, &BTreeMap::new())?;
            }
            docker::require_ready(&containers)?;
            self.manifests.insert(req.root.clone(), manifest);
            if req.json {
                return serde_json::to_string_pretty(&RestartJson {
                    project: name.into(),
                    status: "restarted".into(),
                    id: None,
                })
                .map_err(|e| e.to_string());
            }
            return Ok(format!("{name}: restarted (compose)"));
        }

        let rows = self.registry.list().map_err(|e| e.to_string())?;
        if let Some(id) = self.ids.get(&key).copied() {
            if rows.iter().any(|r| r.id == id) {
                self.registry
                    .stop(id, Duration::from_secs(3))
                    .map_err(|e| e.to_string())?;
            }
            self.ids.remove(&key);
        }

        let cfg = config::load_checked(&svc.path)?;
        let env = cfg.as_ref().map(|c| c.env.clone()).unwrap_or_default();
        let cmd = svc
            .cmd
            .clone()
            .or_else(|| config::resolve_dev_command(&svc.path))
            .ok_or_else(|| format!("{name}: no cmd/commands.dev/scripts.dev"))?;
        let id = self
            .registry
            .start_service(name, &cmd, &svc.path, svc.port)?;
        self.ids.insert(key, id);
        if let Some(check) = &svc.healthcheck {
            run_command(check, &svc.path, &env)?;
        }
        self.manifests.insert(req.root.clone(), manifest);
        if req.json {
            Ok(serde_json::to_string_pretty(&RestartJson {
                project: name.into(),
                status: "restarted".into(),
                id: Some(id),
            })
            .map_err(|e| e.to_string())?)
        } else {
            Ok(format!("{name}: restarted #{id}"))
        }
    }
}

fn serve() -> Result<(), String> {
    let dir = runtime_dir()?;
    // An OS advisory lock is released even after a crash. It prevents a second
    // daemon from unlinking the live daemon's socket during concurrent `up`.
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("supervisor.lock"))
        .map_err(|e| e.to_string())?;
    lock.try_lock()
        .map_err(|e| format!("supervisor already running: {e}"))?;
    let socket = dir.join("dem.sock");
    match std::fs::remove_file(&socket) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.to_string()),
    }
    let listener = UnixListener::bind(&socket).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    let mut state = Supervisor::default();
    for stream in listener.incoming() {
        let mut stream = stream.map_err(|e| e.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| e.to_string())?;
        let mut line = String::new();
        let response = match BufReader::new(&stream).read_line(&mut line) {
            Ok(_) if line.len() <= 65536 => serde_json::from_str::<Request>(&line)
                .map_err(|e| e.to_string())
                .and_then(|req| state.execute(req)),
            Ok(_) => Err("request too large".into()),
            Err(e) => Err(e.to_string()),
        };
        let response = match response {
            Ok(output) => Response {
                output,
                error: None,
            },
            Err(error) => Response {
                output: String::new(),
                error: Some(error),
            },
        };
        let _ = writeln!(
            stream,
            "{}",
            serde_json::to_string(&response).map_err(|e| e.to_string())?
        );
    }
    Ok(())
}

pub fn run(args: &[String]) -> std::process::ExitCode {
    let action = args.first().map(String::as_str).unwrap_or("help");
    if ["help", "--help", "-h"].contains(&action) {
        println!("{USAGE}");
        return std::process::ExitCode::SUCCESS;
    }
    let result = if action == "__serve" {
        serve()
    } else {
        match parse_args(args) {
            Err(msg) if msg == "help" => {
                println!("{USAGE}");
                Ok(())
            }
            Err(e) => Err(e),
            Ok(parsed) if parsed.action == "watch" => loop {
                let req = Request {
                    action: "watch".into(),
                    root: parsed.root.clone(),
                    project: None,
                    json: false,
                };
                match request(&req) {
                    Ok(text) => println!("\x1b[2J\x1b[H{text}"),
                    Err(e) => break Err(e),
                }
                std::thread::sleep(Duration::from_secs(1));
            },
            Ok(parsed) => {
                let req = Request {
                    action: parsed.action,
                    root: parsed.root,
                    project: parsed.project,
                    json: parsed.json,
                };
                request(&req).map(|text| println!("{text}"))
            }
        }
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("dem: {e}");
            std::process::ExitCode::FAILURE
        }
    }
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
        for args in [
            vec![],
            vec!["help".into()],
            vec!["-h".into()],
            vec!["--help".into()],
        ] {
            assert_eq!(code_u8(run(&args)), 0, "args={args:?}");
        }
    }

    #[test]
    fn unknown_and_extra_args_fail() {
        assert_eq!(code_u8(run(&["nope".into()])), 1);
        assert_eq!(
            code_u8(run(&["status".into(), ".".into(), "extra".into()])),
            1
        );
    }

    #[test]
    fn actions_list_matches_issue_40_and_102() {
        for cmd in ["status", "up", "down", "sync", "watch"] {
            assert!(INFRA_ACTIONS.contains(&cmd), "{cmd}");
        }
        for cmd in ["pulse", "restart"] {
            assert!(PROJECT_ACTIONS.contains(&cmd), "{cmd}");
        }
    }

    #[test]
    fn parse_pulse_json_and_restart() {
        let p = parse_args(&["pulse".into(), "parser".into(), "--json".into()]).unwrap();
        assert_eq!(p.action, "pulse");
        assert_eq!(p.project.as_deref(), Some("parser"));
        assert!(p.json);
        assert_eq!(p.root, PathBuf::from("."));

        let p2 = parse_args(&["pulse".into(), "--json".into()]).unwrap();
        assert!(p2.project.is_none());
        assert!(p2.json);

        let r = parse_args(&["restart".into(), "api".into(), "--json".into()]).unwrap();
        assert_eq!(r.action, "restart");
        assert_eq!(r.project.as_deref(), Some("api"));
        assert!(r.json);

        assert!(parse_args(&["restart".into()]).is_err());
        assert!(parse_args(&["restart".into(), "--json".into()]).is_err());
        assert!(parse_args(&["status".into(), "--json".into()]).is_err());
    }

    #[test]
    fn parse_pulse_with_root_path() {
        let p = parse_args(&["pulse".into(), "parser".into(), ".".into(), "--json".into()]).unwrap();
        assert_eq!(p.project.as_deref(), Some("parser"));
        assert_eq!(p.root, PathBuf::from("."));
        assert!(p.json);
    }

    #[test]
    fn pulse_json_flattens_payload() {
        let row = PulseJson {
            project: "parser".into(),
            pulse: PulsePayload {
                summary: "1500 лемм".into(),
                metrics: BTreeMap::from([("parsed".into(), 1500)]),
                eta_seconds: Some(600),
                state: Some("running".into()),
            },
        };
        let text = serde_json::to_string(&row).unwrap();
        assert!(text.contains("\"project\":\"parser\""));
        assert!(text.contains("\"summary\":\"1500 лемм\""));
        assert!(text.contains("\"eta_seconds\":600"));
        let back: PulseJson = serde_json::from_str(&text).unwrap();
        assert_eq!(back, row);
    }

    #[test]
    fn restart_json_shape() {
        let row = RestartJson {
            project: "api".into(),
            status: "restarted".into(),
            id: Some(7),
        };
        let v: serde_json::Value = serde_json::from_str(&serde_json::to_string(&row).unwrap()).unwrap();
        assert_eq!(v["project"], "api");
        assert_eq!(v["status"], "restarted");
        assert_eq!(v["id"], 7);
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

    #[test]
    fn request_serde_backward_compatible() {
        let legacy = r#"{"action":"status","root":"/tmp"}"#;
        let req: Request = serde_json::from_str(legacy).unwrap();
        assert_eq!(req.action, "status");
        assert!(req.project.is_none());
        assert!(!req.json);

        let full = Request {
            action: "pulse".into(),
            root: PathBuf::from("/proj"),
            project: Some("parser".into()),
            json: true,
        };
        let round: Request = serde_json::from_str(&serde_json::to_string(&full).unwrap()).unwrap();
        assert_eq!(round.project.as_deref(), Some("parser"));
        assert!(round.json);
    }

    #[test]
    fn execute_pulse_reads_file_json() {
        let dir = std::env::temp_dir().join(format!(
            "demerzel-cli-pulse-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("orchestrator.json"),
            r#"{
              "name": "parser",
              "type": "parser",
              "pulse": { "file": "pulse.json", "interval_secs": 30 }
            }"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("pulse.json"),
            r#"{"summary":"1500 лемм","metrics":{"parsed":1500},"eta_seconds":600,"state":"running"}"#,
        )
        .unwrap();

        let mut sup = Supervisor::default();
        let out = sup
            .execute(Request {
                action: "pulse".into(),
                root: dir.clone(),
                project: Some("parser".into()),
                json: true,
            })
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["project"], "parser");
        assert_eq!(v["summary"], "1500 лемм");
        assert_eq!(v["eta_seconds"], 600);
        assert_eq!(v["metrics"]["parsed"], 1500);

        let text = sup
            .execute(Request {
                action: "pulse".into(),
                root: dir.clone(),
                project: None,
                json: false,
            })
            .unwrap();
        assert!(text.contains("parser: 1500 лемм"));
        assert!(text.contains("ещё 10 мин"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
