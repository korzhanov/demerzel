# Пульс проекта (#97)

Проект сообщает прогресс длинной работы (парсер, сборка). Demerzel опрашивает источник по интервалу, хранит последнюю успешную запись и кладёт снимок в `summary.json` для виджета ([#98](https://github.com/korzhanov/demerzel/issues/98), [widget-macos.md](widget-macos.md)).

CLI `dem pulse` / `dem restart` и локальный Unix-сокет — [cli.md](cli.md) ([#102](https://github.com/korzhanov/demerzel/issues/102)).

## Конфиг (`orchestrator.json`)

Ровно один источник: `command` **или** `file`. Схема: [orchestrator.schema.json](orchestrator.schema.json).

| Поле | По умолчанию | Смысл |
|---|---|---|
| `command` | — | `sh -c` в корне проекта; **stdout** = JSON пульса |
| `file` | — | Относительный путь к JSON (без `..`, не absolute) |
| `interval_secs` | `30` | Минимальный интервал между опросами |
| `timeout_secs` | `5` | Таймаут команды (для `file` не используется) |

Пример — команда:

```json
{
  "name": "parser",
  "type": "parser",
  "pulse": {
    "command": "bun run pulse",
    "interval_secs": 30,
    "timeout_secs": 5
  }
}
```

Пример — файл:

```json
{
  "name": "parser",
  "type": "parser",
  "pulse": {
    "file": "pulse.json",
    "interval_secs": 15
  }
}
```

Готовый пример парсера: [`examples/parser/`](../examples/parser/).

## Формат JSON

```json
{
  "summary": "1500 новых лемм, 450 проверено",
  "metrics": { "parsed": 1500, "checked": 450 },
  "eta_seconds": 600,
  "state": "running"
}
```

| Поле | Обязательно | Описание |
|---|---|---|
| `summary` | да | Непустой текст для строки виджета |
| `metrics` | нет | Целые счётчики (`parsed`, `checked`, …) |
| `eta_seconds` | нет | Оценка оставшегося времени |
| `state` | нет | Например `running` / `idle` / `done` |

Виджет показывает `name: summary, ещё N мин` (метрики в UI не рисуются, но остаются в `summary.json`).

## Поведение опроса

Модуль: `src-tauri/src/backend/pulse.rs`.

1. Если с последнего успешного опроса прошло меньше `interval_secs` — вернуть кэш.
2. Иначе выполнить команду / прочитать файл, провалидировать JSON.
3. Успех → обновить кэш (`PulseStore`).
4. Ошибка при уже известном кэше → оставить последнюю запись (не затирать).
5. Команда: process group, по таймауту TERM → KILL.

`env` из `orchestrator.json` передаётся в команду пульса.

## Связь с виджетом

Снимок → `summary.projects[].pulse` ([#98](https://github.com/korzhanov/demerzel/issues/98)). Источник опроса — этот документ; отрисовка small/medium — [widget-macos.md](widget-macos.md).
