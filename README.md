# Demerzel

The Shadow Orchestrator of Your Local Stack Empire.

Локальный оркестратор для разработки на macOS: дашборд проектов и подпроектов, статусы задач, распределение портов, запуск агентов для исправления багов, виджет на рабочем столе и голосовые алерты.

## Стек

- Tauri 2 (оболочка и бэкенд на Rust)
- SvelteKit в режиме SPA (интерфейс)
- `tauri-plugin-widgets` (виджет macOS, запасной вариант: Swift WidgetKit)
- SQLite (состояние)
- Bun (пакеты, скрипты, запуск)

Решения зафиксированы в [docs/decisions](docs/decisions). Указатель по всей документации: [docs/README.md](docs/README.md).

## Требования

- macOS (виджет и `say` заточены под него)
- [Bun](https://bun.sh/) 1.4+
- Rust stable + Xcode CLI tools
- Опционально: [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code), [Ollama](https://ollama.com/), Docker Desktop (для `type: docker-compose` в `demerzel.yaml`)

## Запуск

```sh
bun install
bun run tauri dev
```

Сборка приложения: `bun run tauri build`.

Юнит-тесты фронта: `bun test src/lib`.  
Юнит-тесты бэкенда: `cd src-tauri && cargo test --lib`.  
Проверка типов: `bun run check`.

CLI `dem` (после сборки бинаря из `src-tauri`): `dem status|up|down|sync|watch` — см. [docs/cli.md](docs/cli.md).

Состояние рантайма и worktree агентов живут в `~/.orchestrator/` (override: `DEMERZEL_RUNTIME_DIR`, `DEMERZEL_WORKTREES_DIR`).

### Доступ к папкам (macOS)

Если проекты лежат в `Documents` / Desktop / iCloud — выдайте доступ Demerzel: [docs/permissions.md](docs/permissions.md).

## Настройка `orchestrator.json`

В корне **каждого** проекта и подпроекта лежит [`orchestrator.json`](docs/orchestrator.schema.json). Схема валидируется при загрузке.

Минимальный пример:

```json
{
  "name": "api",
  "type": "service",
  "commands": {
    "dev": "bun run dev",
    "test": "bun test",
    "migrate": "bunx prisma migrate deploy"
  },
  "ports": {
    "range": [5100, 5199],
    "default": 5100
  },
  "tasks": {
    "source": "todo"
  },
  "env": {
    "NODE_ENV": "development"
  }
}
```

| Поле | Назначение |
|---|---|
| `name` | Имя проекта (обязательно) |
| `type` | `app` \| `parser` \| `dashboard` \| `service` |
| `commands.dev` | Команда разработки (GUI / `dem up`) |
| `commands.test` | Тесты после прогона агента |
| `commands.migrate` | Миграции при `dem sync` (пусто → пропуск) |
| `ports.range` | Диапазон портов `[start, end]` |
| `ports.default` | Порт по умолчанию внутри диапазона |
| `tasks.source` | `todo` (`TODO.md`) \| `tasks-json` \| `github` \| `notion` |
| `tasks.repository` | `owner/name` для `source=github` |
| `tasks.database_id` | UUID Notion DB для `source=notion` (`NOTION_TOKEN`) |
| `pulse` | Опрос прогресса: `command` **или** `file` ([docs/pulse.md](docs/pulse.md), #97) |
| `env` | Переменные окружения процесса |

Монорепо / несколько сервисов: корневой [`demerzel.yaml`](docs/demerzel-yaml.md) перечисляет сервисы; детали каждого проекта остаются в его `orchestrator.json`.

Настройки приложения (алерты, лимиты агентов, Ollama, iCloud summary, WidgetKit fallback) — в `settings.json` приложения, не в `orchestrator.json`. Подробности: [docs/alerts.md](docs/alerts.md), [docs/agents.md](docs/agents.md), [docs/icloud-summary.md](docs/icloud-summary.md), [docs/widget-macos.md](docs/widget-macos.md), [docs/widgetkit-fallback.md](docs/widgetkit-fallback.md).

## Статус

Версия: см. [`VERSION`](VERSION). План работ — [GitHub Issues](https://github.com/korzhanov/demerzel/issues); фазы через метки `phase-0` … `phase-7`. Phase-7 later (#36–#38) закрыты в 0.36–0.38.

## Документация

Полный указатель: **[docs/README.md](docs/README.md)**.

- [docs/CONVENTIONS.md](docs/CONVENTIONS.md) — ветки, коммиты, SemVer, журналы
- [docs/orchestrator.schema.json](docs/orchestrator.schema.json) — схема `orchestrator.json`
- [docs/demerzel-yaml.md](docs/demerzel-yaml.md) — манифест экосистемы
- [docs/tasks-format.md](docs/tasks-format.md) — `TODO.md` / `tasks.json`
- [docs/cli.md](docs/cli.md) — CLI `dem`
- [docs/agents.md](docs/agents.md) — worktree, Claude / Ollama, ревью
- [docs/alerts.md](docs/alerts.md) — правила и доставка алертов
- [docs/icloud-summary.md](docs/icloud-summary.md) — `summary.json` → iCloud для iPhone
- [docs/widget-macos.md](docs/widget-macos.md) — сборка `.appex`, установка, Widget Gallery
- [docs/widgetkit-fallback.md](docs/widgetkit-fallback.md) — native WidgetKit fallback (#38)
- [docs/permissions.md](docs/permissions.md) — доступ к папкам macOS
- [docs/decisions/](docs/decisions/) — ADR
- [references/](references/README.md) — мокапы и ассеты
- [CHANGELOG.md](CHANGELOG.md) / [MARKETING.md](MARKETING.md)

## Лицензия

Не выбрана.
