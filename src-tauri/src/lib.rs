// The backend modules are pure Rust and do not depend on Tauri. See backend/mod.rs.
pub mod backend;

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use std::path::PathBuf;

use backend::alerts::{AlertEvent, DeliveryDecision, DeliveryGate};
use backend::notify::{self, NotifyPayload, OpenProjectPayload, EXTRA_PROJECT, OPEN_PROJECT_EVENT};
use backend::speak::{self, SaySpeaker};
use backend::registry::{Registry, Row};
use backend::scanner::{self, Project};
use backend::logs::LogLine;
use backend::icloud_sync::{self, SyncResult};
use backend::summary::Summary;
use backend::widget;
use backend::widget_refresh::{Throttle, DEFAULT_MIN_INTERVAL_SECS};
use chrono::Utc;
use serde::Serialize;
use tauri::{Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_widgets::{models::WidgetConfig, WidgetExt};

const APP_GROUP: &str = "group.app.demerzel.orchestrator";

/// Реестр запущенных процессов.
pub struct Processes(pub Mutex<Registry>);

/// Состояние троттлинга виджета, общее для всего приложения.
pub struct WidgetState(pub Mutex<Throttle>);

/// Stateful `DeliveryGate` для алертов (#21–#23).
pub struct AlertGate(pub Mutex<DeliveryGate>);

/// In-memory реестр agent runs для UI ревью (#32).
pub struct AgentRuns(pub Mutex<backend::agents::AgentRunRegistry>);

/// Лимитер параллельных агентов (#29 / #33).
pub struct AgentLimit(pub Mutex<backend::agents::AgentLimiter>);

/// Результат приёма события алерта (для UI / тестов).
#[derive(Debug, Clone, Serialize)]
pub struct IngestAlertResult {
    pub matched: bool,
    pub decision: Option<DeliveryDecisionDto>,
    pub notified: bool,
    pub spoken: bool,
    pub project: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeliveryDecisionDto {
    pub emit: bool,
    pub speak: bool,
    pub notify: bool,
}

impl From<DeliveryDecision> for DeliveryDecisionDto {
    fn from(d: DeliveryDecision) -> Self {
        Self {
            emit: d.emit,
            speak: d.speak,
            notify: d.notify,
        }
    }
}

fn show_os_notification(app: &tauri::AppHandle, payload: &NotifyPayload) -> Result<(), String> {
    let mut builder = app
        .notification()
        .builder()
        .title(payload.title.clone())
        .body(payload.body.clone());
    if let Some(project) = &payload.project {
        builder = builder.extra(EXTRA_PROJECT, project);
    }
    builder.show().map_err(|e| e.to_string())
}

fn focus_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn handle_notification_action(app: &tauri::AppHandle, action_id: &str, project: Option<String>) {
    if action_id != "tap" {
        return;
    }
    focus_main_window(app);
    if let Some(project) = project.filter(|p| !p.is_empty()) {
        let _ = app.emit(OPEN_PROJECT_EVENT, OpenProjectPayload { project });
    }
}

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
    // iCloud sync best-effort: ошибка не блокирует macOS-виджет (#37 ≠ #38).
    let _ = sync_summary_to_icloud(app.clone(), summary.clone());
    refresh_widget(&app, &summary)
}

/// Пишет `summary.json` в iCloud Drive, если `settings.icloud.enabled`.
#[tauri::command]
fn sync_summary_to_icloud(app: tauri::AppHandle, summary: Summary) -> Result<SyncResult, String> {
    let settings = backend::settings::load(&settings_path(&app)?).map_err(|e| e.to_string())?;
    let home = icloud_sync::user_home().ok_or_else(|| "HOME is not set".to_string())?;
    icloud_sync::sync_summary(&home, &settings.icloud, &summary).map_err(|e| e.to_string())
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
fn save_settings(
    app: tauri::AppHandle,
    gate: tauri::State<'_, AlertGate>,
    settings: backend::settings::Settings,
) -> Result<(), String> {
    backend::settings::save(&settings_path(&app)?, &settings).map_err(|e| e.to_string())?;
    // Новая политика доставки; история дедупа сбрасывается вместе с конфигом.
    let mut g = gate.0.lock().map_err(|e| e.to_string())?;
    *g = DeliveryGate::from_config(&settings.alerts);
    Ok(())
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
    if source == "notion" {
        let tasks = task_config.as_ref().ok_or("missing tasks configuration")?;
        return backend::notion_tasks::NotionSource {
            database_id: tasks
                .database_id
                .clone()
                .ok_or("tasks.database_id is required for source=notion")?,
            title_property: tasks.title_property.clone(),
            status_property: tasks.status_property.clone(),
        }
        .fetch(&path);
    }
    backend::tasks::FileSource { root, json: source == "tasks-json" }.fetch(&path)
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn list_agent_runs(
    runs: tauri::State<'_, AgentRuns>,
) -> Result<Vec<backend::agents::AgentRunRecord>, String> {
    let reg = runs.0.lock().map_err(|e| e.to_string())?;
    Ok(reg.list())
}

#[tauri::command]
fn get_agent_run(
    runs: tauri::State<'_, AgentRuns>,
    id: String,
) -> Result<backend::agents::AgentRunRecord, String> {
    let reg = runs.0.lock().map_err(|e| e.to_string())?;
    reg.get(&id)
        .cloned()
        .ok_or_else(|| format!("agent run not found: {id}"))
}

/// Запись / обновление run (сиды для UI и будущий запуск из задачи #33).
#[tauri::command]
fn upsert_agent_run(
    runs: tauri::State<'_, AgentRuns>,
    record: backend::agents::AgentRunRecord,
) -> Result<(), String> {
    let mut reg = runs.0.lock().map_err(|e| e.to_string())?;
    reg.upsert(record)
}

#[tauri::command]
fn accept_agent_run(
    runs: tauri::State<'_, AgentRuns>,
    id: String,
) -> Result<backend::agents::AcceptAgentRunResult, String> {
    let mut reg = runs.0.lock().map_err(|e| e.to_string())?;
    reg.accept(&id)
}

#[tauri::command]
fn discard_agent_run(
    runs: tauri::State<'_, AgentRuns>,
    id: String,
) -> Result<backend::agents::AgentRunRecord, String> {
    let mut reg = runs.0.lock().map_err(|e| e.to_string())?;
    reg.discard(&id)
}

/// Запуск агента из задачи (#33 / #30): статус `in_progress`, worktree; CLI в фоне.
#[tauri::command]
fn start_agent_from_task(
    app: tauri::AppHandle,
    runs: tauri::State<'_, AgentRuns>,
    limit: tauri::State<'_, AgentLimit>,
    task: backend::tasks::Task,
    runner: Option<String>,
) -> Result<backend::agents::StartAgentRunResult, String> {
    use backend::agents::{
        complete_agent_run, prepare_agent_from_task, task_prompt_text, AgentRunner,
        DEFAULT_CLAUDE_TIMEOUT, DEFAULT_OLLAMA_TIMEOUT,
    };
    use std::thread;

    let settings = backend::settings::load(&settings_path(&app)?).map_err(|e| e.to_string())?;
    let runner = match runner.as_deref() {
        Some(raw) => AgentRunner::parse(raw)?,
        None => settings.agents.runner,
    };
    let ollama_model = settings.agents.ollama_model.clone();
    let timeout = match runner {
        AgentRunner::Claude => DEFAULT_CLAUDE_TIMEOUT,
        AgentRunner::Ollama => DEFAULT_OLLAMA_TIMEOUT,
    };

    let project_path = PathBuf::from(&task.project);
    let prompt = task_prompt_text(&task);
    let prepared = {
        let mut reg = runs.0.lock().map_err(|e| e.to_string())?;
        let mut lim = limit.0.lock().map_err(|e| e.to_string())?;
        prepare_agent_from_task(
            &task,
            &project_path,
            &mut reg,
            &mut lim,
            Some("HEAD"),
            None,
            runner,
        )?
    };

    if prepared.queued {
        return Ok(prepared);
    }

    let run_id = prepared.run.id.clone();
    let app_handle = app.clone();
    thread::spawn(move || {
        let runs_state = app_handle.state::<AgentRuns>();
        let limit_state = app_handle.state::<AgentLimit>();
        let Ok(mut reg) = runs_state.0.lock() else {
            return;
        };
        let Ok(mut lim) = limit_state.0.lock() else {
            return;
        };
        if let Err(e) = complete_agent_run(
            &run_id,
            &prompt,
            &mut reg,
            &mut lim,
            timeout,
            None,
            Some(ollama_model.as_str()),
        ) {
            eprintln!("[agents] complete_agent_run failed: {e}");
        }
    });

    Ok(prepared)
}

/// Оценивает событие → gate → OS notify (#22) и `say` (#23) по флагам решения.
#[tauri::command]
fn ingest_alert_event(
    app: tauri::AppHandle,
    gate: tauri::State<'_, AlertGate>,
    event: AlertEvent,
) -> Result<IngestAlertResult, String> {
    let settings = backend::settings::load(&settings_path(&app)?).map_err(|e| e.to_string())?;
    let project = notify::project_hint(&event);
    let Some(candidate) = backend::alerts::evaluate(&settings.alerts.rules, &event) else {
        return Ok(IngestAlertResult {
            matched: false,
            decision: None,
            notified: false,
            spoken: false,
            project,
        });
    };
    let decision = {
        let mut g = gate.0.lock().map_err(|e| e.to_string())?;
        g.decide(&candidate, Utc::now())
    };
    let notified = if let Some(payload) = notify::maybe_notify(&candidate, &decision) {
        show_os_notification(&app, &payload)?;
        true
    } else {
        false
    };
    // Ошибка `say` не валит ingest: gate уже учёл speak; логируем в stderr.
    let spoken = match speak::deliver_speak(
        &candidate,
        &decision,
        &settings.alerts.voice,
        &SaySpeaker,
    ) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[alerts] say failed: {e}");
            false
        }
    };
    Ok(IngestAlertResult {
        matched: true,
        decision: Some(decision.into()),
        notified,
        spoken,
        project: notify::project_hint(&candidate.event).or(project),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_widgets::init())
        .manage(WidgetState(Mutex::new(Throttle::new(DEFAULT_MIN_INTERVAL_SECS))))
        .manage(Processes(Mutex::new(Registry::default())))
        .manage(AlertGate(Mutex::new(DeliveryGate::new_default())))
        .manage(AgentRuns(Mutex::new(backend::agents::AgentRunRegistry::new())))
        .manage(AgentLimit(Mutex::new(backend::agents::AgentLimiter::from_defaults())))
        .setup(|app| {
            let handle = app.handle().clone();
            app.notification().on_action(move |performed| {
                let project = performed
                    .notification()
                    .and_then(|n| notify::project_from_extra(n.extra()));
                handle_notification_action(&handle, performed.action_id(), project);
            })?;
            // Подтянуть quiet/dedupe и agents limiter из settings при старте.
            if let Ok(path) = settings_path(app.handle())
                && let Ok(settings) = backend::settings::load(&path)
            {
                if let Ok(mut gate) = app.state::<AlertGate>().0.lock() {
                    *gate = DeliveryGate::from_config(&settings.alerts);
                }
                if let Ok(mut lim) = app.state::<AgentLimit>().0.lock() {
                    *lim = backend::agents::AgentLimiter::new(settings.agents);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            refresh_widget_from_summary,
            sync_summary_to_icloud,
            scan_projects,
            get_settings,
            save_settings,
            resolve_dev_command,
            start_process,
            stop_process,
            list_processes,
            process_logs,
            list_ports,
            project_tasks,
            list_agent_runs,
            get_agent_run,
            upsert_agent_run,
            accept_agent_run,
            discard_agent_run,
            start_agent_from_task,
            ingest_alert_event
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
