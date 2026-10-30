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
| `dem help` | Справка |

`ROOT` по умолчанию — текущая директория.

## Supervisor

Состояние процессов держит фоновый supervisor на Unix-сокете `~/.orchestrator/runtime/dem.sock` (override: `DEMERZEL_RUNTIME_DIR`). Команды `up`, `sync`, `status` и `watch` поднимают его при необходимости; `down` требует уже запущенный supervisor.

Лог supervisor: `~/.orchestrator/runtime/supervisor.log` (или `$DEMERZEL_RUNTIME_DIR/supervisor.log`).

## Зависимости

- Менеджер процессов / реестр (#8) и логи (#9).
- Манифест экосистемы [`demerzel.yaml`](demerzel-yaml.md): process и `type: docker-compose` (рантайм Compose — #42 / `docker.rs`). Для одного проекта достаточно `orchestrator.json`.
