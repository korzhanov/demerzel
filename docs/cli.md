# CLI `dem` / `demerzel`

Бинарники: `dem` (`[[bin]]`) и `demerzel` (пакет / GUI). Команды локальной инфраструктуры читают `demerzel.yaml` в корне или, если его нет, один сервис из `orchestrator.json` / `commands.dev`.

## Команды

| Команда | Назначение |
|---|---|
| `dem status [ROOT]` | Состояние процессов, Docker-сервисов и фоновых задач |
| `dem up [ROOT]` | Запуск всей локальной инфраструктуры |
| `dem down [ROOT]` | Остановка сервисов, поднятых через `up` |
| `dem sync [ROOT]` | Перезапуск упавших + `commands.migrate` |
| `dem watch [ROOT]` | Живой стрим логов и CPU/RSS (раз в секунду) |
| `dem pulse [PROJECT] [--json] [ROOT]` | Пульс прогресса проекта (#97 / #102) |
| `dem restart <PROJECT> [--json] [ROOT]` | Перезапуск одного сервиса (#102) |
| `dem help` | Справка |

`ROOT` по умолчанию — текущая директория. `PROJECT` — имя сервиса из `demerzel.yaml` или `name` из `orchestrator.json`.

## Пульс и перезапуск (#102)

Опираются на контракт опроса ([pulse.md](pulse.md)). Ходят в тот же Unix-сокет, что и `status` / `up` — единый локальный API для CLI и будущих App Intents. Сети наружу нет.

| Команда | Поведение |
|---|---|
| `dem pulse` | Все проекты с `pulse` в `orchestrator.json`; человекочитаемая строка `name: summary, ещё N мин` |
| `dem pulse parser --json` | JSON: `{ "project", "summary", "metrics?", "eta_seconds?", "state?" }` |
| `dem restart parser` | Stop + start process-сервиса или `docker compose restart` |
| `dem restart parser --json` | `{ "project", "status": "restarted", "id?" }` |

Без `--json` несколько пульсов выводятся построчно; с `--json` — один объект или массив.

## Миграции (`commands.migrate`)

`dem sync` не знает конкретный ORM. Для каждого process-сервиса читается [`orchestrator.json`](orchestrator.schema.json) → `commands.migrate` и запускается как shell-команда в каталоге сервиса с `env` из конфига.

| Ситуация | Поведение |
|---|---|
| Поле задано и непустое | Команда выполняется; stdout/stderr попадают в ответ `sync` и в лог `watch` как `[name migrate] …` |
| Поля нет, пусто или только пробелы | Миграции пропускаются: `name migrate: no commands.migrate; skipped` |
| Команда завершилась с ошибкой | `dem sync` возвращает ошибку (сервис в статусе ошибки) |

Docker Compose-сервисы (`type: docker-compose`) миграции через `commands.migrate` не вызывают — у них свой health/ready цикл.

Пример:

```json
{
  "name": "api",
  "type": "service",
  "commands": {
    "dev": "bun run dev",
    "migrate": "bunx prisma migrate deploy"
  }
}
```

## Supervisor / локальный API

Состояние процессов держит фоновый supervisor на Unix-сокете `~/.orchestrator/runtime/dem.sock` (override: `DEMERZEL_RUNTIME_DIR`). Протокол — одна JSON-строка запроса / ответа на соединение:

```json
{ "action": "pulse", "root": "/path/to/ecosystem", "project": "parser", "json": true }
```

| Поле | Смысл |
|---|---|
| `action` | `up` / `down` / `sync` / `status` / `watch` / `pulse` / `restart` |
| `root` | Каталог с `demerzel.yaml` или `orchestrator.json` |
| `project` | Имя сервиса (нужен для `restart`; для `pulse` опционален) |
| `json` | Машинный ответ для `pulse` / `restart` |

Ответ: `{ "output": "…", "error": null }` или `{ "output": "", "error": "…" }`.

Команды `up`, `sync`, `status`, `watch`, `pulse` и `restart` поднимают supervisor при необходимости; `down` требует уже запущенный supervisor.

Лог supervisor: `~/.orchestrator/runtime/supervisor.log` (или `$DEMERZEL_RUNTIME_DIR/supervisor.log`).

## Зависимости

- Менеджер процессов / реестр (#8) и логи (#9).
- Манифест экосистемы [`demerzel.yaml`](demerzel-yaml.md): process и `type: docker-compose` (рантайм Compose — #42 / `docker.rs`). Для одного проекта достаточно `orchestrator.json`.
- Контракт пульса (#97): [pulse.md](pulse.md).
