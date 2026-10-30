// The backend modules are pure Rust and do not depend on Tauri. See backend/mod.rs.
pub mod backend;

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use std::path::PathBuf;

use backend::registry::{Registry, Row};
use backend::scanner::{self, Project};
use backend::logs::LogLine;
use backend::summary::Summary;
use backend::widget;
use backend::widget_refresh::{Throttle, DEFAULT_MIN_INTERVAL_SECS};
use tauri::Manager;
use tauri_plugin_widgets::{models::WidgetConfig, WidgetExt};

const APP_GROUP: &str = "group.app.demerzel.orchestrator";

/// Реестр запущенных процессов.
pub struct Processes(pub Mutex<Registry>);

/// Состояние троттлинга виджета, общее для всего приложения.
pub struct WidgetState(pub Mutex<Throttle>);

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Обновляет виджет по сводке, если содержимое изменилось и прошёл интервал.
/// Возвращает `true`, если обновление выполнено.
pub fn refresh_widget(app: &tauri::AppHandle, summary: &Summary) -> Result<bool, String> {
    let json = widget::small(summary);
    let content = json.to_string();
    let state = app.state::<WidgetState>();
    let mut throttle = state.0.lock().map_err(|e| e.to_string())?;
    let now = now_secs();
    if !throttle.should_refresh(now, &content) {
        return Ok(false);
    }
    let cfg: WidgetConfig = serde_json::from_value(json).map_err(|e| e.to_string())?;
    let handle = app.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let w = handle.widget();
        let res = w
            .set_widget_config(&cfg, APP_GROUP, widget::WIDGET_ID, true)
            .map_err(|e| e.to_string())
            .and_then(|_| w.reload_all_timelines().map_err(|e| e.to_string()));
        let _ = tx.send(res);
    })
    .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())??;
    throttle.mark(now, &content);
    Ok(true)
}

#[tauri::command]
fn refresh_widget_from_summary(app: tauri::AppHandle, summary: Summary) -> Result<bool, String> {
    refresh_widget(&app, &summary)
}

fn settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|d| d.join(backend::settings::SETTINGS_FILE))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_settings(app: tauri::AppHandle) -> Result<backend::settings::Settings, String> {
    backend::settings::load(&settings_path(&app)?).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_settings(app: tauri::AppHandle, settings: backend::settings::Settings) -> Result<(), String> {
    backend::settings::save(&settings_path(&app)?, &settings).map_err(|e| e.to_string())
}

#[tauri::command]
fn scan_projects(roots: Vec<String>) -> Result<Vec<Project>, String> {
    let mut all = Vec::new();
    for root in roots {
        let path = PathBuf::from(&root);
        let found = scanner::scan(&path).map_err(|e| {
            scanner::access_hint(&e, &path).unwrap_or_else(|| format!("{}: {e}", path.display()))
        })?;
        all.extend(found);
    }
    Ok(all)
}

#[tauri::command]
fn resolve_dev_command(path: String) -> Option<String> {
    backend::config::resolve_dev_command(&PathBuf::from(path))
}

#[tauri::command]
fn start_process(
    processes: tauri::State<'_, Processes>,
    project: String,
    path: String,
    command: String,
) -> Result<u64, String> {
    let cwd = PathBuf::from(&path);
    // Пустое поле или устаревший дефолт UI «bun run dev» → commands.dev с диска.
    let cmd = {
        let trimmed = command.trim();
        let use_disk = trimmed.is_empty() || trimmed == "bun run dev";
        if use_disk {
            if let Some(resolved) = backend::config::resolve_dev_command(&cwd) {
                resolved
            } else if trimmed.is_empty() {
                return Err(format!(
                    "нет commands.dev в {} и нет scripts.dev в package.json — укажите команду в поле «команда»",
                    backend::config::CONFIG_FILE
                ));
            } else {
                trimmed.to_string()
            }
        } else {
            trimmed.to_string()
        }
    };
    let mut reg = processes.0.lock().map_err(|e| e.to_string())?;
    // Диапазон из orchestrator.json → аренда → env PORT (#13).
    reg.start_configured(&project, &cmd, &cwd)
}

#[tauri::command]
fn stop_process(processes: tauri::State<'_, Processes>, id: u64) -> Result<Option<backend::process::Status>, String> {
    let mut reg = processes.0.lock().map_err(|e| e.to_string())?;
    reg.stop(id, std::time::Duration::from_secs(5)).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_processes(processes: tauri::State<'_, Processes>) -> Result<Vec<Row>, String> {
    let mut reg = processes.0.lock().map_err(|e| e.to_string())?;
    reg.list().map_err(|e| e.to_string())
}

#[tauri::command]
fn process_logs(processes: tauri::State<'_, Processes>, id: u64) -> Result<Option<Vec<LogLine>>, String> {
    let reg = processes.0.lock().map_err(|e| e.to_string())?;
    Ok(reg.logs(id))
}

#[tauri::command]
fn list_ports(processes: tauri::State<'_, Processes>) -> Result<Vec<backend::ports::PortRow>, String> {
    processes.0.lock().map_err(|e| e.to_string())?.port_inventory()
}

#[tauri::command]
fn project_tasks(path: String) -> Result<Vec<backend::tasks::Task>, String> {
    use backend::tasks::TaskSource;
    let root = PathBuf::from(&path);
    let cfg = backend::config::load_checked(&root)?;
    let task_config = cfg.and_then(|c| c.tasks);
    let source = task_config.as_ref().map(|t| t.source.as_str()).unwrap_or("todo");
    if source == "github" {
        let tasks = task_config.as_ref().ok_or("missing tasks configuration")?;
        return backend::github_tasks::GithubSource {
            repository: tasks.repository.clone().ok_or("tasks.repository must be owner/name")?,
            label: tasks.label.clone(),
        }.fetch(&path);
    }
    backend::tasks::FileSource { root, json: source == "tasks-json" }.fetch(&path)
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_widgets::init())
        .manage(WidgetState(Mutex::new(Throttle::new(DEFAULT_MIN_INTERVAL_SECS))))
        .manage(Processes(Mutex::new(Registry::default())))
        .invoke_handler(tauri::generate_handler![
            greet,
            refresh_widget_from_summary,
            scan_projects,
            get_settings,
            save_settings,
            resolve_dev_command,
            start_process,
            stop_process,
            list_processes,
            process_logs,
            list_ports,
            project_tasks
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
