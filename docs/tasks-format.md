# Формат файлов задач

Demerzel читает задачи проекта из файлов в корне проекта (или подпроекта). Источник задаётся в `orchestrator.json` → `tasks.source`:

| `source` | Файл | Адаптер |
|---|---|---|
| `todo` | `TODO.md` | `FileSource { json: false }` |
| `tasks-json` | `tasks.json` | `FileSource { json: true }` |
| `github` | — | GitHub Issues (#18) |

Нормализованная модель — `Task` (#16): `id`, `project`, `title`, `status`, `source`, `url?`.
Статусы на проводе: `todo`, `in_progress`, `done`, `blocked`.

При старте агента из задачи (#33) статус переводится в `in_progress` (`mark_task_in_progress`): для `todo` / `tasks-json` пишется в файл; для GitHub remote не меняется. Закрытие (`done`) после ревью агента — только вручную.

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

## GitHub Issues (`source: "github"`)

Адаптер `GithubSource` в `github_tasks.rs` (#18). Только REST через `gh api` (сессия `gh auth login` / токен Keychain). GraphQL не используется.

В `orchestrator.json`:

```json
{
  "name": "demerzel",
  "type": "app",
  "tasks": {
    "source": "github",
    "repository": "owner/name",
    "label": "tasks"
  }
}
```

| Поле | Обязательно | Описание |
|---|---|---|
| `repository` | да (при fetch) | `owner/name`; путь/URL отклоняются |
| `label` | нет | фильтр метки; без него — все открытые issues |

Поведение:

- Запрос: `GET /repos/{owner}/{name}/issues?state=open&per_page=100` (+ `labels=`).
- Записи с полем `pull_request` отбрасываются.
- `open` → `todo`, `closed` → `done` (на случай нормализации вне списка open).
- Id: `github:{owner}/{name}#{number}`; `source` = `github`; `url` = `html_url`.
- Кэш: ETag + `If-None-Match` (ответ `304` → кэш); повторный REST не чаще раза в 60 с на endpoint (лимит запросов).
- Пагинация до 100 страниц; иначе ошибка с просьбой сузить `label`.
