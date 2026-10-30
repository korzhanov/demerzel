# Алерты: движок правил

Phase-4, issue #20. Правила живут в **конфиге приложения** (`settings.json` → `alerts`), не в `demerzel.yaml` / `orchestrator.json`.

## Уровни важности

| Severity   | Голос (`say`, #23) | Системное уведомление (#22) | UI |
|------------|--------------------|-----------------------------|----|
| `critical` | да                 | да                          | да |
| `important`| нет                | да                          | да |
| `info`     | нет                | нет                         | да |

Доставка, дедуп и тихие часы — отдельные issues (#21–#23). Этот модуль только **матчит событие → `AlertCandidate`**.

## Правила по умолчанию

| id | Событие | Условие | Severity |
|----|---------|---------|----------|
| `process_exited_nonzero` | `process_exited` | код ≠ 0 | critical |
| `port_conflict` | `port_conflict` | always | critical |
| `agent_failed` | `agent_failed` | always | important |
| `tests_failed` | `tests_failed` | always | important |

Чистый выход процесса (код 0) кандидата не даёт.

## Контракт

- `AlertRule { id, kind, when, severity, enabled }`
- `AlertEvent` — входящее событие рантайма
- `evaluate(rules, event) -> Option<AlertCandidate>` — первое подходящее включённое правило
- `should_speak` / `should_notify` — подсказки для #22/#23 по severity

Пример фрагмента `settings.json`:

```json
{
  "scan_roots": ["/Users/me/code"],
  "alerts": {
    "rules": [
      {
        "id": "process_exited_nonzero",
        "kind": "process_exited",
        "when": { "op": "exit_code_non_zero" },
        "severity": "critical",
        "enabled": true
      }
    ]
  }
}
```

Отсутствующее поле `alerts` подставляет правила по умолчанию.

## Код

- Rust: `src-tauri/src/backend/alerts.rs`
- Настройки: `backend/settings.rs` → `Settings.alerts`
- Зеркало UI-хелперов: `src/lib/alerts.ts`
