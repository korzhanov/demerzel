# Алерты: движок правил, политика доставки и системные уведомления

Phase-4, issues #20–#22. Правила и политика живут в **конфиге приложения** (`settings.json` → `alerts`), не в `demerzel.yaml` / `orchestrator.json`.

## Уровни важности

| Severity   | Голос (`say`, #23) | Системное уведомление (#22) | UI |
|------------|--------------------|-----------------------------|----|
| `critical` | да*                | да*                         | да |
| `important`| нет                | да*                         | да |
| `info`     | нет                | нет                         | да |

\* С учётом политики доставки (#21): дедуп, троттлинг голоса, тихие часы.

Модуль **матчит** событие → `AlertCandidate`, затем `DeliveryGate` решает `emit` / `speak` / `notify`. При `notify=true` слой #22 показывает OS-уведомление (`tauri-plugin-notification`). Голос `say` — #23.

## Системные уведомления (#22)

| Шаг | Поведение |
|-----|-----------|
| Gate | Показ только если `DeliveryDecision.notify` (тихие часы → только `critical`) |
| Текст | Заголовок `Demerzel · Критично/Важно`, тело = короткий `message` кандидата |
| Клик | Фокус окна + событие `alerts:open-project` с `project` (для `process_exited`) |
| Focus/DND | Публичного API статуса Focus для non-communication apps нет — см. [ADR-002](./decisions/002-focus-mode.md); опираемся на quiet hours + фильтр macOS |

Команда Tauri: `ingest_alert_event(event)` → `{ matched, decision, notified, project }`.

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
- `notify::maybe_notify(candidate, decision) -> Option<NotifyPayload>`
- `notify::project_hint` / `project_from_extra` — deep-link проекта при клике

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

- Rust: `src-tauri/src/backend/alerts.rs`, `backend/notify.rs`
- Tauri: `ingest_alert_event`, plugin `tauri-plugin-notification`, capability `notification:default`
- Настройки: `backend/settings.rs` → `Settings.alerts`
- Зеркало UI-хелперов: `src/lib/alerts.ts`
- ADR Focus Mode: `docs/decisions/002-focus-mode.md`
- Wireframe: `references/alerts-notify-wireframe.md`
