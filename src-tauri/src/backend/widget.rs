//! Конфиги виджетов из Summary (#25, #98).
//!
//! Чистая функция: строит JSON-IR для tauri-plugin-widgets. Записи в WidgetKit
//! делает слой Tauri, здесь только форма данных.
//! Пульс проектов (#98): строка summary + ETA; reload снаружи только при смене текста IR.
//! Полоса прогресса (#99): если в `metrics` пульса есть пара `done` и `total`, виджет рисует
//! полосу `done / total`. Без такой пары — только текст.

use super::summary::{format_pulse_line, ProjectPulse, ProjectStatus, Summary};
use serde_json::{json, Map, Value};

pub const WIDGET_ID: &str = "default";
/// Сколько строк пульса в small / medium.
pub const PULSE_LINES_SMALL: usize = 1;
pub const PULSE_LINES_MEDIUM: usize = 3;
/// Сколько проектов показывает medium.
pub const MEDIUM_PROJECTS: usize = 5;

const BG: &str = "#0d0d12";
const COLOR_MUTED: &str = "#8888aa";
const COLOR_TEXT: &str = "#e0e0e0";
const COLOR_PULSE: &str = "#b0b0cc";
const COLOR_OK: &str = "#4CAF50";
const COLOR_FAIL: &str = "#E53935";
const COLOR_IDLE: &str = "#5c5c7a";

/// Цвет акцента: зелёный, если упавших нет, красный иначе.
pub fn tint(failing: usize) -> &'static str {
    if failing == 0 { COLOR_OK } else { COLOR_FAIL }
}

fn text(content: impl Into<String>, size: u32, color: &str) -> Value {
    json!({ "type": "text", "content": content.into(), "fontSize": size, "color": color })
}

fn bold(content: impl Into<String>, size: u32, color: &str) -> Value {
    json!({
        "type": "text",
        "content": content.into(),
        "fontSize": size,
        "fontWeight": "bold",
        "color": color
    })
}

/// Доля выполнения из `metrics.done / metrics.total`, в диапазоне 0..=1.
pub fn pulse_progress(pulse: &ProjectPulse) -> Option<f64> {
    let metrics = pulse.metrics.as_ref()?;
    let done = *metrics.get("done")?;
    let total = *metrics.get("total")?;
    (total > 0).then(|| (done as f64 / total as f64).clamp(0.0, 1.0))
}

fn running_total(summary: &Summary) -> usize {
    summary.projects.iter().map(|p| p.running).sum()
}

/// Главная строка: сколько с ошибкой, иначе сколько запущено, иначе «всё спокойно».
fn headline(summary: &Summary) -> String {
    if summary.failing_total > 0 {
        format!("{} с ошибкой", summary.failing_total)
    } else if running_total(summary) > 0 {
        format!("{} запущено", running_total(summary))
    } else {
        "Всё спокойно".into()
    }
}

fn ports_label(summary: &Summary) -> String {
    if summary.port_conflicts == 0 {
        "порты в порядке".into()
    } else {
        format!("конфликт портов: {}", summary.port_conflicts)
    }
}

fn project_state(p: &ProjectStatus) -> &'static str {
    if p.failing > 0 {
        "с ошибкой"
    } else if p.running > 0 {
        "запущен"
    } else {
        "остановлен"
    }
}

fn project_color(p: &ProjectStatus) -> &'static str {
    if p.failing > 0 {
        COLOR_FAIL
    } else if p.running > 0 {
        COLOR_OK
    } else {
        COLOR_IDLE
    }
}

/// Строки пульса (и полосы прогресса, где они есть) для первых `limit` проектов.
fn pulse_nodes(summary: &Summary, limit: usize) -> Vec<Value> {
    summary
        .projects_with_pulse()
        .take(limit)
        .flat_map(|(name, pulse)| {
            let mut nodes = Vec::new();
            if let Some(value) = pulse_progress(pulse) {
                nodes.push(json!({
                    "type": "progress",
                    "value": value,
                    "tint": COLOR_OK,
                    "label": name
                }));
            }
            nodes.push(text(format_pulse_line(name, pulse), 12, COLOR_PULSE));
            nodes
        })
        .collect()
}

/// Маленький виджет: главный статус, состояние портов и первый пульс.
pub fn small(summary: &Summary) -> Value {
    let mut children = vec![
        text("DEMERZEL", 10, COLOR_MUTED),
        bold(headline(summary), 24, tint(summary.failing_total)),
        text(ports_label(summary), 12, COLOR_TEXT),
    ];
    children.extend(pulse_nodes(summary, PULSE_LINES_SMALL));
    json!({
        "small": { "type": "vstack", "padding": 14, "background": BG, "children": children }
    })
}

/// Средний виджет: статус, список проектов с точками состояния, пульс и счётчики задач.
pub fn medium(summary: &Summary) -> Value {
    let mut children = vec![
        text("DEMERZEL", 10, COLOR_MUTED),
        bold(headline(summary), 16, tint(summary.failing_total)),
    ];
    for p in summary.projects.iter().take(MEDIUM_PROJECTS) {
        children.push(text(
            format!("● {}  ·  {}", p.name, project_state(p)),
            13,
            project_color(p),
        ));
    }
    children.extend(pulse_nodes(summary, PULSE_LINES_MEDIUM));
    children.push(text(
        format!(
            "Задачи: {} к выполнению · {} в работе · агентов: {}",
            summary.tasks_todo, summary.tasks_in_progress, summary.agents_running
        ),
        11,
        COLOR_MUTED,
    ));
    json!({
        "medium": { "type": "vstack", "padding": 14, "background": BG, "children": children }
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
/// Прогресс входит в отпечаток, чтобы полоса обновлялась вместе с текстом.
pub fn pulse_fingerprint(summary: &Summary) -> String {
    summary
        .projects_with_pulse()
        .map(|(name, pulse)| {
            format!("{}|{:?}", format_pulse_line(name, pulse), pulse_progress(pulse))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::summary::{ProjectPulse, ProjectStatus, Summary};
    use std::collections::BTreeMap;

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
            metrics: None,
        });
        s.projects.push(ProjectStatus {
            name: "web".into(),
            running: 1,
            failing: 0,
            pulse: Some(ProjectPulse {
                summary: "сборка".into(),
                eta_seconds: Some(90),
                state: None,
                metrics: None,
            }),
        });
        s
    }

    #[test]
    fn small_shows_headline_and_tint() {
        let v = small(&sample(3));
        let headline = &v["small"]["children"][1];
        assert_eq!(headline["content"], "3 с ошибкой");
        assert_eq!(headline["color"], tint(3));
        assert_eq!(tint(0), COLOR_OK);
    }

    #[test]
    fn headline_says_calm_when_nothing_runs() {
        let mut s = sample(0);
        s.projects[0].running = 0;
        s.recompute();
        assert_eq!(headline(&s), "Всё спокойно");
    }

    #[test]
    fn medium_lists_projects_with_state_and_tasks_footer() {
        let mut s = sample(0);
        s.tasks_todo = 3;
        s.tasks_in_progress = 1;
        s.agents_running = 2;
        let v = medium(&s);
        let children = v["medium"]["children"].as_array().unwrap();
        assert!(children
            .iter()
            .any(|c| c["content"].as_str().unwrap_or("").contains("api  ·  запущен")));
        let footer = children.last().unwrap()["content"].as_str().unwrap();
        assert_eq!(footer, "Задачи: 3 к выполнению · 1 в работе · агентов: 2");
    }

    #[test]
    fn project_dot_is_red_when_failing() {
        let v = medium(&sample(2));
        let children = v["medium"]["children"].as_array().unwrap();
        let row = children
            .iter()
            .find(|c| c["content"].as_str().unwrap_or("").starts_with("● api"))
            .unwrap();
        assert_eq!(row["color"], COLOR_FAIL);
    }

    #[test]
    fn progress_comes_from_done_and_total_and_is_clamped() {
        let mut s = with_pulse();
        s.projects[0].pulse.as_mut().unwrap().metrics = Some(BTreeMap::from([
            ("done".into(), 50),
            ("total".into(), 10),
        ]));
        let v = small(&s);
        let children = v["small"]["children"].as_array().unwrap();
        let bar = children.iter().find(|c| c["type"] == "progress").unwrap();
        assert_eq!(bar["value"], 1.0);
        assert_eq!(bar["label"], "api");
    }

    #[test]
    fn no_progress_bar_without_metrics() {
        let v = small(&with_pulse());
        let children = v["small"]["children"].as_array().unwrap();
        assert!(children.iter().all(|c| c["type"] != "progress"));
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
