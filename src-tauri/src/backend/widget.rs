//! Конфиги виджетов из Summary (#25, #98).
//!
//! Чистая функция: строит JSON-IR для tauri-plugin-widgets. Записи в WidgetKit
//! делает слой Tauri, здесь только форма данных.
//! Пульс проектов (#98): строка summary + ETA; reload снаружи только при смене текста IR.

use super::summary::{format_pulse_line, Summary};
use serde_json::{json, Map, Value};

pub const WIDGET_ID: &str = "default";
/// Сколько строк пульса в small / medium.
pub const PULSE_LINES_SMALL: usize = 1;
pub const PULSE_LINES_MEDIUM: usize = 3;

/// Цвет акцента: зелёный, если упавших нет, красный иначе.
pub fn tint(failing: usize) -> &'static str {
    if failing == 0 { "#4CAF50" } else { "#E53935" }
}

fn pulse_text_nodes(summary: &Summary, limit: usize) -> Vec<Value> {
    summary
        .projects_with_pulse()
        .take(limit)
        .map(|(name, pulse)| {
            json!({
                "type": "text",
                "content": format_pulse_line(name, pulse),
                "fontSize": 12,
                "color": "#b0b0cc"
            })
        })
        .collect()
}

/// Маленький виджет: число упавших сервисов, полоса портов, строка пульса.
pub fn small(summary: &Summary) -> Value {
    let failing = summary.failing_total;
    let label = if summary.port_conflicts == 0 { "Ports ok" } else { "Port conflicts" };
    let progress = (failing as f64 / 10.0).clamp(0.0, 1.0);
    let mut children = vec![
        json!({ "type": "text", "content": "Demerzel", "fontSize": 12, "color": "#8888aa" }),
        json!({ "type": "text", "content": format!("{failing} failing"), "fontSize": 28, "fontWeight": "bold", "color": "#ffffff" }),
        json!({ "type": "progress", "value": progress, "tint": tint(failing), "label": label }),
    ];
    children.extend(pulse_text_nodes(summary, PULSE_LINES_SMALL));
    json!({
        "small": {
            "type": "vstack",
            "padding": 12,
            "background": "#0d0d12",
            "children": children
        }
    })
}

/// Средний виджет: список проектов со статусами, счётчики задач и пульс.
pub fn medium(summary: &Summary) -> Value {
    let rows: Vec<Value> = summary
        .projects
        .iter()
        .take(5)
        .map(|p| {
            json!({
                "type": "text",
                "content": format!("{}  ·  {} running, {} failing", p.name, p.running, p.failing),
                "fontSize": 13,
                "color": if p.failing == 0 { "#e0e0e0" } else { "#E53935" }
            })
        })
        .collect();
    let mut children = vec![json!({
        "type": "text", "content": "Demerzel", "fontSize": 12, "color": "#8888aa"
    })];
    children.extend(rows);
    children.extend(pulse_text_nodes(summary, PULSE_LINES_MEDIUM));
    children.push(json!({
        "type": "text",
        "content": format!("Tasks: {} todo · {} in progress", summary.tasks_todo, summary.tasks_in_progress),
        "fontSize": 12,
        "color": "#8888aa"
    }));
    json!({
        "medium": { "type": "vstack", "padding": 12, "background": "#0d0d12", "children": children }
    })
}

/// Полный IR small+medium для `set_widget_config` (#98).
pub fn config(summary: &Summary) -> Value {
    let mut map = Map::new();
    if let Some(obj) = small(summary).as_object() {
        for (k, v) in obj {
            map.insert(k.clone(), v.clone());
        }
    }
    if let Some(obj) = medium(summary).as_object() {
        for (k, v) in obj {
            map.insert(k.clone(), v.clone());
        }
    }
    Value::Object(map)
}

/// Текст пульса, по которому решается «изменился ли виджет» (без updated_at).
pub fn pulse_fingerprint(summary: &Summary) -> String {
    summary
        .projects_with_pulse()
        .map(|(name, pulse)| format_pulse_line(name, pulse))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::summary::{ProjectPulse, ProjectStatus, Summary};

    fn sample(failing: usize) -> Summary {
        let mut s = Summary::new("t");
        s.projects.push(ProjectStatus {
            name: "api".into(),
            running: 1,
            failing,
            pulse: None,
        });
        s.recompute();
        s
    }

    fn with_pulse() -> Summary {
        let mut s = sample(0);
        s.projects[0].pulse = Some(ProjectPulse {
            summary: "1500 новых лемм, 450 проверено".into(),
            eta_seconds: Some(600),
            state: Some("running".into()),
        });
        s.projects.push(ProjectStatus {
            name: "web".into(),
            running: 1,
            failing: 0,
            pulse: Some(ProjectPulse {
                summary: "сборка".into(),
                eta_seconds: Some(90),
                state: None,
            }),
        });
        s
    }

    #[test]
    fn small_shows_failing_count_and_tint() {
        let v = small(&sample(3));
        let text = v["small"]["children"][1]["content"].as_str().unwrap();
        assert_eq!(text, "3 failing");
        assert_eq!(tint(3), "#E53935");
        assert_eq!(tint(0), "#4CAF50");
    }

    #[test]
    fn medium_lists_projects_and_tasks() {
        let v = medium(&sample(0));
        let children = v["medium"]["children"].as_array().unwrap();
        assert!(children.iter().any(|c| c["content"].as_str().unwrap_or("").starts_with("api")));
        assert!(children.last().unwrap()["content"].as_str().unwrap().starts_with("Tasks:"));
    }

    #[test]
    fn progress_is_clamped() {
        let v = small(&sample(50));
        assert_eq!(v["small"]["children"][2]["value"], 1.0);
    }

    #[test]
    fn small_and_medium_include_pulse_lines() {
        let s = with_pulse();
        let small_ir = small(&s);
        let small_children = small_ir["small"]["children"].as_array().unwrap();
        let pulse_small: Vec<_> = small_children
            .iter()
            .filter_map(|c| c["content"].as_str())
            .filter(|t| t.contains("лемм") || t.contains("сборка"))
            .collect();
        assert_eq!(pulse_small.len(), 1);
        assert!(pulse_small[0].contains("ещё 10 мин"));

        let medium_ir = medium(&s);
        let medium_children = medium_ir["medium"]["children"].as_array().unwrap();
        let pulse_medium: Vec<_> = medium_children
            .iter()
            .filter_map(|c| c["content"].as_str())
            .filter(|t| t.contains("лемм") || t.contains("сборка"))
            .collect();
        assert_eq!(pulse_medium.len(), 2);
    }

    #[test]
    fn config_merges_sizes() {
        let v = config(&with_pulse());
        assert!(v.get("small").is_some());
        assert!(v.get("medium").is_some());
    }

    #[test]
    fn pulse_fingerprint_stable_for_same_text() {
        let a = with_pulse();
        let mut b = with_pulse();
        b.updated_at = "other".into();
        assert_eq!(pulse_fingerprint(&a), pulse_fingerprint(&b));
        b.projects[0].pulse.as_mut().unwrap().summary = "другое".into();
        assert_ne!(pulse_fingerprint(&a), pulse_fingerprint(&b));
    }
}
