# Документация Demerzel

Указатель по `docs/`: решения (ADR), форматы файлов, схемы и операционные гайды. Корневой обзор и быстрый старт — в [README.md](../README.md).

Справочные документы пока на русском. Обзор и быстрый старт есть на [английском](../README.md), [русском](../README.ru.md) и [немецком](../README.de.md).

## Старт и конвенции

| Документ | О чём |
|---|---|
| [../README.md](../README.md) | Что это, запуск, `orchestrator.json` |
| [CONVENTIONS.md](CONVENTIONS.md) | Ветки, Conventional Commits, SemVer, журналы |
| [permissions.md](permissions.md) | Доступ к папкам на macOS |

## Конфиги и схемы

| Документ | О чём |
|---|---|
| [orchestrator.schema.json](orchestrator.schema.json) | JSON Schema `orchestrator.json` (проект / подпроект) |
| [demerzel.schema.json](demerzel.schema.json) | JSON Schema `demerzel.yaml` (экосистема) |
| [demerzel-yaml.md](demerzel-yaml.md) | Манифест экосистемы, process / docker-compose |
| [pulse.md](pulse.md) | Контракт пульса: команда / файл, опрос, metrics (#97) |
| [tasks-format.md](tasks-format.md) | `TODO.md`, `tasks.json`, источники задач |

## Операционные гайды

| Документ | О чём |
|---|---|
| [cli.md](cli.md) | CLI `dem` / `demerzel`: status, up, down, sync, watch, pulse, restart (#102) |
| [agents.md](agents.md) | Worktree, Claude CLI, Ollama, лимиты, ревью (#27–#33) |
| [alerts.md](alerts.md) | Правила алертов, notify / speak, quiet hours (#20–#23) |
| [icloud-summary.md](icloud-summary.md) | `summary.json` → iCloud Drive для iPhone (#37) |
| [widget-macos.md](widget-macos.md) | Сборка `.appex`, embed PlugIns, Widget Gallery (#103), пульс в small/medium (#98); источник опроса — [pulse.md](pulse.md) (#97) |
| [widgetkit-fallback.md](widgetkit-fallback.md) | Native Swift WidgetKit fallback, App Group (#38) |
| [siri-app-intents.md](siri-app-intents.md) | App Intents / Siri: AskDemerzelStatus + RestartProcess, dem.sock (#101) |
| [releasing.md](releasing.md) | Подпись обновлений, GitHub Releases и первый updater-enabled install |

## ADR (Architecture Decision Records)

Каталог: [decisions/](decisions/).

| ADR | Тема |
|---|---|
| [001-stack.md](decisions/001-stack.md) | Стек и виджет macOS (`tauri-plugin-widgets`) |
| [001-report.md](decisions/001-report.md) | Отчёт по спайку виджета: как повторить, чеклист |
| [002-focus-mode.md](decisions/002-focus-mode.md) | Режим фокуса / shadow |
| [003-siri-app-intents.md](decisions/003-siri-app-intents.md) | App Intents / Siri Shortcuts (spike go → #101) |
| [003-report.md](decisions/003-report.md) | Отчёт спайка Siri: dem.sock probe, фразы, чеклист |

## Референсы и журналы

| Путь | О чём |
|---|---|
| [../CHANGELOG.md](../CHANGELOG.md) | Технический журнал по версиям |
| [../MARKETING.md](../MARKETING.md) | Короткие формулировки для соцсетей |
| [`VERSION`](../VERSION) | Текущая SemVer |

## План фаз

Работа идёт через [GitHub Issues](https://github.com/korzhanov/demerzel/issues).

Метки фаз: `phase-0` … `phase-7`.

| Фаза | Содержание (кратко) | Типичные issues |
|---|---|---|
| 0–1 | Каркас, сканер, конфиг | закрыты в ранних PR |
| 2 | Порты, CLI, `demerzel.yaml` | #8–#15, #42–#43 |
| 3 | Задачи (модель, файлы, GitHub) | #16–#19 |
| 4 | Алерты | #20–#23 |
| 5 | Виджет / summary | #25–#26 |
| 6 | Агенты (worktree → ревью) | #27–#33 |
| 7 | Later / fallback | #36 Notion (0.36.0), #37 iCloud (0.37.0), #38 WidgetKit fallback (0.38.0) |
| 8 | Siri / App Intents | #101 intents (0.47.0); spike #100 go |

Правила доски и статусов — в [CONVENTIONS.md](CONVENTIONS.md#github-project).
