# Алерты: движок правил, политика доставки и системные уведомления

Phase-4, issues #20–#23. Правила и политика живут в **конфиге приложения** (`settings.json` → `alerts`), не в `demerzel.yaml` / `orchestrator.json`.

## Уровни важности

| Severity   | Голос (`say`, #23) | Системное уведомление (#22) | UI |
|------------|--------------------|-----------------------------|----|
| `critical` | да*                | да*                         | да |
| `important`| нет                | да*                         | да |
| `info`     | нет                | нет                         | да |

\* С учётом политики доставки (#21): дедуп, троттлинг голоса, тихие часы.

Модуль **матчит** событие → `AlertCandidate`, затем `DeliveryGate` решает `emit` / `speak` / `notify`. При `notify=true` слой #22 показывает OS-уведомление (`tauri-plugin-notification`). При `speak=true` слой #23 вызывает `say -v <voice>` с короткой фразой из `alerts.voice`.

## Голос `say` (#23)

| Шаг | Поведение |
|-----|-----------|
| Gate | Произношение только если `DeliveryDecision.speak` (critical; тихие часы → speak off; ≤1/мин) |
| Голос | `alerts.voice.name` (по умолчанию `Milena`) |
| Фраза | Шаблон из `alerts.voice.phrases.*` + плейсхолдеры `{project}`, `{code}`, `{port}`, `{run_id}` |
| Вызов | `say -v <name> <phrase>` через трейт `VoiceSpeaker` / `SaySpeaker` (мок в тестах) |
| Ошибка | Сбой `say` логируется; ingest не падает |

Шаблоны по умолчанию:

| Событие | Фраза |
|---------|--------|
| `process_exited` | `{user}, внимание. {project} упал, код {code}.` |
| `port_conflict` | `{user}, внимание. Порт {port} занят.` |
| `agent_failed` | `{user}, внимание. Агент {run_id} не справился.` |
| `tests_failed` | `{user}, внимание. Тесты {run_id} упали.` |

## Системные уведомления (#22)

| Шаг | Поведение |
|-----|-----------|
| Gate | Показ только если `DeliveryDecision.notify` (тихие часы → только `critical`) |
| Текст | Заголовок `Demerzel · Критично/Важно`, тело = короткий `message` кандидата |
| Клик | Фокус окна + событие `alerts:open-project` с `project` (для `process_exited`) |
| Focus/DND | Публичного API статуса Focus для non-communication apps нет — см. [ADR-002](./decisions/002-focus-mode.md); опираемся на quiet hours + фильтр macOS |

Команда Tauri: `ingest_alert_event(event)` → `{ matched, decision, notified, spoken, project, autofix_offer?, autofix_started? }`.

## Автофикс по алерту (#96)

При `process_exited` (код ≠ 0), `port_conflict`, `tests_failed` можно предложить или запустить агента в worktree. `agent_failed` не запускает автофикс (петля).

| Режим | Где | Поведение |
|-------|-----|-----------|
| `off` | по умолчанию | ничего |
| `suggest` | settings / проект | UI-баннер «Исправить» в один клик |
| `auto` | settings / проект | сразу `prepare` + CLI в фоне |

Конфиг: `settings.json` → `agents.autofix`; override на проект — `orchestrator.json` → `agents.autofix`. Приоритет: проект → settings → `off`.

Контекст промпта: сообщение алерта + команда + хвост логов (до 80 строк) + `git diff` рабочего дерева / прошлого run. Лимитер (#29) и ревью (#31–#32) как у запуска из задачи: результат → `needs_review` / `failed`, не в main.

Событие UI: `alerts:autofix-offer`. Команда: `start_agent_from_alert(project_path, prompt, runner?)`.

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
- `VoiceConfig { name, phrases }` — голос и шаблоны `say`
- `notify::maybe_notify(candidate, decision) -> Option<NotifyPayload>`
- `speak::maybe_speak(candidate, decision, voice) -> Option<SpeakPayload>`
- `speak::VoiceSpeaker` / `SaySpeaker` — вызов `say` (мок в тестах)
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
    "voice": {
      "name": "Milena",
      "user_name": "Олег",
      "user_pronunciation": "",
      "phrases": {
        "process_exited": "{user}, внимание. {project} упал, код {code}.",
        "port_conflict": "{user}, внимание. Порт {port} занят."
      }
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

Отсутствующее поле `alerts` (и недостающие поля доставки / `voice`) подставляет значения по умолчанию.

## Код

- Rust: `src-tauri/src/backend/alerts.rs`, `backend/notify.rs`, `backend/speak.rs`
- Tauri: `ingest_alert_event`, plugin `tauri-plugin-notification`, capability `notification:default`
- Настройки: `backend/settings.rs` → `Settings.alerts`
- Зеркало UI-хелперов: `src/lib/alerts.ts`
- ADR Focus Mode: `docs/decisions/002-focus-mode.md`
- Wireframe: `references/alerts-notify-wireframe.md`, `references/alerts-speak-wireframe.md`
