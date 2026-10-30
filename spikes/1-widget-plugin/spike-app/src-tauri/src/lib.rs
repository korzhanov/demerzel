use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use tauri_plugin_widgets::{models::WidgetConfig, WidgetExt};

const APP_GROUP: &str = "group.app.demerzel.orchestrator";
const WIDGET_ID: &str = "default";

static FAILING: AtomicU32 = AtomicU32::new(0);

fn widget_config(failing: u32) -> tauri_plugin_widgets::Result<WidgetConfig> {
    let tint = if failing == 0 { "#4CAF50" } else { "#E53935" };
    let label = if failing == 0 { "Ports ok" } else { "Attention" };
    let progress = (failing as f64 / 10.0).clamp(0.0, 1.0);

    let value = serde_json::json!({
        "small": {
            "type": "vstack",
            "padding": 12,
            "background": "#0d0d12",
            "children": [
                { "type": "text", "content": "Demerzel", "fontSize": 12, "color": "#8888aa" },
                {
                    "type": "text",
                    "content": format!("{failing} failing"),
                    "fontSize": 28,
                    "fontWeight": "bold",
                    "color": "#ffffff"
                },
                {
                    "type": "progress",
                    "value": progress,
                    "tint": tint,
                    "label": label
                }
            ]
        }
    });

    serde_json::from_value(value).map_err(|e| {
        tauri_plugin_widgets::Error::new(format!("widget config json: {e}"))
    })
}

fn push_widget(app: &tauri::AppHandle) -> Result<(), String> {
    let failing = FAILING.load(Ordering::Relaxed);
    let cfg = widget_config(failing).map_err(|e| e.to_string())?;
    let w = app.widget();
    // skip_reload=true: write only; reload explicitly after (main-thread FFI).
    let outcome = w
        .set_widget_config(&cfg, APP_GROUP, WIDGET_ID, true)
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("Operation not permitted") || msg.contains("os error 1") {
                format!(
                    "{msg}\nHint: host must be ad-hoc signed without App Sandbox, and the \
                     extension container must already exist under ~/Library/Containers/. \
                     Install to /Applications, open once, then run scripts/sign-macos.sh \
                     and relaunch. Team ID → switch transport to appGroup."
                )
            } else {
                msg
            }
        })?;
    eprintln!(
        "[spike] tick failing={failing} written={} skip={:?}",
        outcome.written, outcome.skip
    );
    w.reload_all_timelines().map_err(|e| e.to_string())?;
    Ok(())
}

fn bump_failing() -> u32 {
    let next = (FAILING.load(Ordering::Relaxed) + 1) % 10;
    FAILING.store(next, Ordering::Relaxed);
    next
}

fn push_on_main(app: &tauri::AppHandle) -> Result<(), String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let app_clone = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(push_widget(&app_clone));
    })
    .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())?
}

/// Spike command: bump failing counter and refresh the widget from Rust.
#[tauri::command]
fn tick(app: tauri::AppHandle) -> Result<u32, String> {
    let next = bump_failing();
    push_on_main(&app)?;
    Ok(next)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_widgets::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![tick])
        .setup(|app| {
            let handle = app.handle().clone();
            if let Err(e) = push_widget(&handle) {
                eprintln!("[spike] initial widget push failed: {e}");
            }

            let handle = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_secs(10));
                    let next = bump_failing();
                    if let Err(e) = push_on_main(&handle) {
                        eprintln!("[spike] tick push failed (failing={next}): {e}");
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
