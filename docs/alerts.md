# Алерты: движок правил и политика доставки

Phase-4, issues #20–#21. Правила и политика живут в **конфиге приложения** (`settings.json` → `alerts`), не в `demerzel.yaml` / `orchestrator.json`.

## Уровни важности

| Severity   | Голос (`say`, #23) | Системное уведомление (#22) | UI |
|------------|--------------------|-----------------------------|----|
| `critical` | да*                | да*                         | да |
| `important`| нет                | да*                         | да |
| `info`     | нет                | нет                         | да |

\* С учётом политики доставки (#21): дедуп, троттлинг голоса, тихие часы.

Доставка `notify` / `say` — отдельные issues (#22–#23). Модуль **матчит** событие → `AlertCandidate`, затем `DeliveryGate` решает `emit` / `speak` / `notify`.

## Правила по умолчанию

| id | Событие | Условие | Severity |
|----|---------|---------|----------|
| `process_exited_nonzero` | `process_exited` | код ≠ 0 | critical |
| `port_conflict` | `port_conflict` | always | critical |
| `agent_failed` | `agent_failed` | always | important |
| `tests_failed` | `tests_failed` | always | important |

Чистый выход процесса (код 0) кандидата не даёт.

## Политика доставки (#21)

| Правило | По умолчанию | Эффект |
|---------|--------------|--------|
| Дедуп | окно **10 мин** (`dedupe_window_secs: 600`) | Одинаковый ключ события схлопывается в один emit |
| Троттлинг голоса | **1 раз / мин** (`voice_min_interval_secs: 60`) | `speak=false`, emit/notify сохраняются |
| Тихие часы | **23:00–08:00** `Europe/Berlin` | Голос выкл.; notify только для `critical` |

Ключ дедупа:

- `process_exited:{project}`
- `port_conflict:{port}`
- `agent_failed:{run_id}`
- `tests_failed:{run_id}`

Окно тихих часов через полночь: `start > end` → quiet, если время ≥ start **или** < end.

## Контракт

- `AlertRule { id, kind, when, severity, enabled }`
- `AlertEvent` — входящее событие рантайма
- `evaluate(rules, event) -> Option<AlertCandidate>` — первое подходящее включённое правило
- `should_speak` / `should_notify` — базовые подсказки по severity
- `DeliveryGate::decide(candidate, now) -> DeliveryDecision { emit, speak, notify }`
- `QuietHoursConfig { enabled, start, end }` + `timezone`

Пример фрагмента `settings.json`:

```json
{
  "scan_roots": ["/Users/me/code"],
  "alerts": {
    "dedupe_window_secs": 600,
    "voice_min_interval_secs": 60,
    "timezone": "Europe/Berlin",
    "quiet_hours": {
      "enabled": true,
      "start": "23:00",
      "end": "08:00"
    },
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

Отсутствующее поле `alerts` (и недостающие поля доставки) подставляет значения по умолчанию.

## Код

- Rust: `src-tauri/src/backend/alerts.rs`
- Настройки: `backend/settings.rs` → `Settings.alerts`
- Зеркало UI-хелперов: `src/lib/alerts.ts`
