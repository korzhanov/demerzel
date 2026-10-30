# Манифест экосистемы `demerzel.yaml`

Корневой манифест монорепо (или папки со стеком). Описывает **какие сервисы** входят в экосистему и как их запускать. Параметры каждого проекта (`commands`, `ports`, `tasks`, `env`) остаются в его собственном [`orchestrator.json`](orchestrator.schema.json).

Машинночитаемая схема: [`demerzel.schema.json`](demerzel.schema.json). Парсер и валидация: `src-tauri/src/backend/ecosystem.rs`.

## Связь с `orchestrator.json`

| Файл | Уровень | Содержание |
|---|---|---|
| `demerzel.yaml` | экосистема (корень) | список сервисов: `path`, `cmd`, `port`, `healthcheck`, опционально `type: docker-compose` |
| `orchestrator.json` | проект / подпроект | имя, тип, команды, диапазон портов, источник задач, env |

Если `demerzel.yaml` отсутствует, CLI строит один сервис из `orchestrator.json` / `commands.dev` в `ROOT` (см. [cli.md](cli.md)).

## Что не входит в манифест

Режим тихой работы (`mode: shadow`) и уведомления (`notifications: desktop`) задаются в **настройках приложения**, не в `demerzel.yaml`. Неизвестные поля отвергаются (`deny_unknown_fields`).

## Поля

Корень:

| Поле | Тип | Обязательно | Описание |
|---|---|---|---|
| `services` | map name → Service | да | Непустой словарь сервисов |

Сервис:

| Поле | Тип | Обязательно | Описание |
|---|---|---|---|
| `path` | path | да | Каталог проекта относительно корня манифеста (или абсолютный) |
| `cmd` | string | нет | Команда запуска процесса; иначе берётся `commands.dev` / эвристика из проекта |
| `port` | u16 ≥ 1 | нет | Явный порт; не должен дублироваться в манифесте; если в проекте задан `ports.range`, порт должен попадать в диапазон |
| `healthcheck` | string | нет | Команда проверки здоровья (непустая, если указана) |
| `type` | `process` \| `docker-compose` | нет | По умолчанию процесс (`process`) |
| `file` | path | для compose | Файл Compose относительно `path` |
| `service` | string | для compose | Имя сервиса в Compose (не начинается с `-`) |

Правила:

- `type: process` (или без `type`) — поля `file` / `service` запрещены.
- `type: docker-compose` — обязательны непустые `file` и `service`. Рантайм Compose — отдельный контракт (#42).
- Пустые `cmd` / `healthcheck` (только пробелы) — ошибка.

## Пример

```yaml
services:
  api:
    path: ./apps/api
    cmd: bun run dev
    port: 5100
    healthcheck: curl -sf http://127.0.0.1:5100/health
  web:
    path: ./apps/web
    port: 5173
  db:
    path: ./infra
    type: docker-compose
    file: compose.yaml
    service: postgres
```

## Валидация при загрузке

1. YAML → `Manifest` (неизвестные ключи — ошибка).
2. Непустой `services`, уникальные ненулевые `port`.
3. Канонизация `path` относительно корня; каталог должен существовать.
4. Для compose: `file` резолвится относительно `path` сервиса.
5. Если у проекта есть `ports.range` в `orchestrator.json` — резерв диапазона и проверка `port` манифеста.
