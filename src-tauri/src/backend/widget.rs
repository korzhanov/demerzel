//! Конфиги виджетов из Summary (#25).
//!
//! Чистая функция: строит JSON-IR для tauri-plugin-widgets. Записи в WidgetKit
//! делает слой Tauri, здесь только форма данных.

use super::summary::Summary;
use serde_json::{json, Value};

pub const WIDGET_ID: &str = "default";

/// Цвет акцента: зелёный, если упавших нет, красный иначе.
pub fn tint(failing: usize) -> &'static str {
    if failing == 0 { "#4CAF50" } else { "#E53935" }
}

/// Маленький виджет: число упавших сервисов и полоса портов.
pub fn small(summary: &Summary) -> Value {
    let failing = summary.failing_total;
    let label = if summary.port_conflicts == 0 { "Ports ok" } else { "Port conflicts" };
    let progress = (failing as f64 / 10.0).clamp(0.0, 1.0);
    json!({
        "small": {
            "type": "vstack",
            "padding": 12,
            "background": "#0d0d12",
            "children": [
                { "type": "text", "content": "Demerzel", "fontSize": 12, "color": "#8888aa" },
                { "type": "text", "content": format!("{failing} failing"), "fontSize": 28, "fontWeight": "bold", "color": "#ffffff" },
                { "type": "progress", "value": progress, "tint": tint(failing), "label": label }
            ]
        }
    })
}

/// Средний виджет: список проектов со статусами и счётчики задач.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::summary::{ProjectStatus, Summary};

    fn sample(failing: usize) -> Summary {
        let mut s = Summary::new("t");
        s.projects.push(ProjectStatus { name: "api".into(), running: 1, failing });
        s.recompute();
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
}
