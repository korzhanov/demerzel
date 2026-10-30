# Формат файлов задач

Demerzel читает задачи проекта из файлов в корне проекта (или подпроекта). Источник задаётся в `orchestrator.json` → `tasks.source`:

| `source` | Файл | Адаптер |
|---|---|---|
| `todo` | `TODO.md` | `FileSource { json: false }` |
| `tasks-json` | `tasks.json` | `FileSource { json: true }` |
| `github` | — | GitHub Issues (#18) |

Нормализованная модель — `Task` (#16): `id`, `project`, `title`, `status`, `source`, `url?`.
Статусы на проводе: `todo`, `in_progress`, `done`, `blocked`.

## TODO.md

Строки-чекбоксы Markdown. Вложенность (отступ) допускается; код в fenced-блоках (`` ``` `` / `~~~`) игнорируется.

| Маркер | Статус |
|---|---|
| `- [ ] title` | `todo` |
| `- [~] title` | `in_progress` |
| `- [x] title` / `- [X] title` | `done` |
| `- [!] title` | `blocked` |

Пустой заголовок и неизвестный маркер (например `- []`) пропускаются.

Идентификатор: `{project}:todo:{номер_строки}` (1-based).  
`source` = `todo`, `url` = путь к `TODO.md`.

Пример:

````md
# Backlog

- [ ] Настроить сканер
  - [~] Парсер чекбоксов
- [x] Модель Task
- [!] Ждём API ключ

```md
- [ ] это в code fence — не задача
```
````

## tasks.json

Массив объектов модели `Task`. Обязательны непустые уникальные `id` и `title`.
Поля `project`, `source` и `url` при чтении перезаписываются адаптером.

Идентификатор после чтения: `{project}:json:{id_из_файла}`.  
`source` = `tasks-json`, `url` = путь к `tasks.json`.

Пример:

```json
[
  {
    "id": "setup-scanner",
    "project": "",
    "title": "Настроить сканер",
    "status": "todo",
    "source": "",
    "url": null
  },
  {
    "id": "parse-md",
    "project": "",
    "title": "Парсер чекбоксов",
    "status": "in_progress",
    "source": "",
    "url": null
  }
]
```

Ошибки: файл не читается, JSON невалиден, пустой `id`/`title`, дубликат `id`.
