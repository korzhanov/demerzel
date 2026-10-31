# Changelog

## 0.55.9 — 2026-10-31

- UI: убран бейдж «Открыто из уведомления» — фокус проекта только выделением в списке.

## 0.55.8 — 2026-10-31

- UI «Агенты»: модалка без top-layer (не перекрывает тултипы/Cursor overlays); Switch с заметным primary; drag за grip + `dataTransfer`.

## 0.55.7 — 2026-10-31

- UI «Агенты»: список в духе Xcode — номер + drag, switch вкл/выкл, кнопка «Установить»; стрелки ↑↓ убраны.

## 0.55.6 — 2026-10-31

- UI: убраны дублирующие лого и заголовок над strip — бренд только на баннере.

## 0.55.5 — 2026-10-31

- UI: strip-баннер внутри `<header>` вместе с лого и панелью действий — одна шапка.

## 0.55.4 — 2026-10-31

- UI: панель действий (агенты, виджеты, логи, вид, тема) перенесена на strip-баннер; в шапке остаются лого и название.

## 0.55.3 — 2026-10-31

- Clippy: упрощён boolean в `ports::listeners` (`nonminimal_bool`).

## 0.55.2 — 2026-10-31

- Агенты (#140): таблица `config/agent-install.json`, UI «Агенты» (приоритет / enabled / Install → Terminal.app), `agents.catalog`, `hermes` + `grok` (unverified), Cursor binary `agent`, Gemini с пометкой про Antigravity.

## 0.55.1 — 2026-10-31

- Сканер видит подпроекты-скрипты без package.json: маркеры `auto-run.json` и `Makefile` (как `parser` в 100words). Закрывает часть #153.

## 0.55.0 — 2026-10-31

- Автозадачи, этап A (#153): `discover_automations` находит кандидатов в проекте — скрипты `package.json`, `[project.scripts]` из `pyproject.toml`, цели `Makefile`, файл `auto-run.json`. Только чтение, ничего не запускает. Тесты на каждый источник.

## 0.54.9 — 2026-10-31

- Rust: `cargo clippy -D warnings` проходит (отчёт `reports/mac-check-20261031-1516.md`): свернуты вложенные `if`, убраны лишние ссылки и `Default`-присваивания. Поведение не менялось.

## 0.54.8 — 2026-10-31

- `scripts/mac-check.sh`: проверка на Mac (bun, cargo test, clippy, PATH для процессов из #133) с отчётом в `reports/` и опцией `--push`. Для обмена результатами между итерациями.

## 0.54.7 — 2026-10-31

- Скриншоты сами собирают фронтенд перед снимком, поэтому всегда отражают текущий код. Если сборка или Playwright недоступны, шаг пропускается.

## 0.54.6 — 2026-10-31

- Тесты обновляют скриншоты: `bun test` после unit-тестов запускает `scripts/screenshots.mjs` и пишет `docs/screenshots/*.png` (колонки и вкладки, тёмная и светлая тема). Без Playwright шаг пропускается. Команда отдельно: `bun run screenshots`.

## 0.54.5 — 2026-10-31

- Иконка: плитка взята точно по границам рамки, углы прозрачные (как у системных иконок macOS). Пересобраны `src-tauri/icons` и `static/app-icon.png`. Продолжение #139.

## 0.54.4 — 2026-10-31

- UI: тултипы выровнены по кнопке и не уходят за край окна; стрелка больше не смещена. Частично #137 (проверено на тёмной теме).

## 0.54.3 — 2026-10-31

- Иконка приложения пересобрана по сетке macOS: изображение 824 px в холсте 1024 с полями, без обрезки. Закрывает #139.

## 0.54.2 — 2026-10-31

- UI: кнопка «Открыть источник» переносит текст и не выходит за карточку задачи. Закрывает #135.

## 0.54.1 — 2026-10-31

- Процессы получают PATH с ~/.bun/bin, /opt/homebrew/bin и другими стандартными каталогами: из приложения больше не падает `Exited(127)`. Закрывает #133.

## 0.54.0 — 2026-10-31

- Исправлено: «Редактор» и «Открыть источник» снова работают — разрешение `opener:allow-open-path` в capabilities. Закрывает #134.

## 0.53.7 — 2026-10-31

- UI: таблица портов компактнее — PID скрыт на узких экранах, действие без лишнего текста. Закрывает #124.

## 0.53.6 — 2026-10-31

- UI: иконка приложения (из `references/media`) в шапке окна. Закрывает #123.

## 0.53.5 — 2026-10-31

- UI: системные процессы macOS и служебные хелперы IDE помечаются «системный», а не «чужой». Закрывает #122.

## 0.53.4 — 2026-10-31

- Заголовок окна «Demerzel» вместо «demerzel-app». Закрывает #121.

## 0.53.3 — 2026-10-31

- UI: поле команды запуска на всю ширину под кнопками, не обрезается. Закрывает #120.

## 0.53.2 — 2026-10-31

- UI: карточки задач не выходят за колонку (перенос длинных слов, две колонки на средней ширине). Закрывает #119.

## 0.53.1 — 2026-10-31

- UI: заголовки задач из GitHub без сырого Markdown и URL (`cleanTaskTitle`). Закрывает #118.

## 0.53.0 — 2026-10-31

- UI: «Проекты» и «Порты» объединены — таблица портов под деревом проектов в левой панели; вкладка «Проекты и порты» в режиме «Вкладки». Закрывает #117.

## 0.52.0 — 2026-10-31

- UI мониторинга: тултипы поверх `<dialog>` (`portalDisabled` / portal `to`), контрастные чекбоксы + light/dark темы, кнопка установки виджетов (`install_widgets`), шапка и сводка на иконках с tooltip/title.

## 0.51.0 — 2026-10-31

- Корни скана: системный dirpicker вместо textarea; дерево папок (expand/collapse), чекбоксы + «все/ничего», per-folder rules с приоритетом вложенных; статус оркестрации и CTA Setup (`orchestrator.json`); иконки + tooltip (shadcn). Backend: `scan_entries`, `list_dir_children`, `get_orchestration`, `init_orchestrator`, `tauri-plugin-dialog`.

## 0.50.1 — 2026-10-31

- Иконка приложения: `src-tauri/icons/*` из `references/mockups/src/icon.jpg` (замена дефолтной Tauri).
- Корневые папки сканирования и автофикс вынесены из узкого сайдбара в модалку «Папки»; команда запуска — у focus проекта. Заголовок окна: Demerzel.
- Панель логов — отдельный слой справа: кнопка «Логи» в шапке, скрытие, ресайз ширины; колонки снова projects + main.

## 0.50.0 — 2026-10-31

- UI ближе к `references/mockups` (#114): баннер strip, шапка DEMERZEL + icon, сводка «Проектов / Упало / Конфликт портов / Задач», сайдбар со status-dots, focus выбранного проекта (процессы/порты/задачи), логи в стиле мокапа; палитра s2.css (`--status-ok`, window edge). Закрывает #114.

## 0.49.0 — 2026-10-31

- Автофикс по алерту (#96): режимы `off` / `suggest` / `auto` (`agents.autofix`, override в `orchestrator.json`), из crash / port conflict / tests failed — агент в worktree с хвостом логов + командой + diff; лимитер и ревью как у #27–#33 (результат не в main). Tauri `start_agent_from_alert`, событие `alerts:autofix-offer`, баннер «Исправить». Закрывает #96.

## 0.48.0 — 2026-10-31

- Реестр CLI-агентов (#95): детект `claude` / `codex` / `gemini` / `aider` / `opencode` / `cursor-agent` / `ollama` по PATH + `--version` (без headless-сессии), подсказки установки, единый `AgentRunner`, агент по умолчанию и на проект (`orchestrator.json` → `agents.runner`). Tauri `list_available_agents` / `resolve_project_agent_runner`. Headless по-прежнему Claude/Ollama. Закрывает #95.

## 0.47.0 — 2026-10-31

- App Intents в продукте (#101): `AskDemerzelStatus` + `RestartProcess` (с подтверждением) в `DemerzelAppIntents.appex`; транспорт `dem.sock`; корень экосистемы в App Group `siri-config.json` из `scan_roots`. Сборка `macos-siri/build-siri.sh` в `beforeBundleCommand`. Контракт/тесты `siri-bridge`. Закрывает #101.

## 0.46.0 — 2026-10-31

- Спайк Siri / App Intents (#100): ADR-003 **go** — Swift App Shortcut «Спроси Demerzel …», entity проект, dialog+snippet; транспорт `dem.sock` (не App Group / не CLI primary). Эталон `spikes/2-siri-app-intents/`, probe `dem-sock-probe.swift`, docs `siri-app-intents.md`, контракт+тесты `siri-bridge`. Далее #101. Закрывает #100.

## 0.45.0 — 2026-10-31

- CLI `dem pulse [проект] [--json]` и `dem restart <проект> [--json]` через supervisor `dem.sock` (локальный API для App Intents, без сети). Опрос пульса через `PulseStore` (#97); compose — `docker compose restart`. Docs `cli.md` / `pulse.md`. Закрывает #102.

## 0.44.0 — 2026-10-31

- Контракт пульса проекта (#97): `pulse` в `orchestrator.json` (`command` или `file`, `interval_secs`, `timeout_secs`), JSON `{summary, metrics?, eta_seconds?, state?}`, модуль `backend/pulse` с таймаутом команды и `PulseStore` (последняя запись). Схема, docs `pulse.md`, пример `examples/parser/`. Снимок совместим с виджетом (#98); CLI `dem pulse` — #102. Закрывает #97.

## 0.43.0 — 2026-10-31

- Пульс проектов в виджете: опциональный `pulse` в `summary.json` (summary + ETA), строки в small (1) / medium (до 3), `widget::config` пушит оба размера; reload только при изменении текста IR. Зеркало в native WidgetKit fallback и `src/lib/pulse.ts`. Docs: `docs/widget-macos.md`. Источник опроса (#97) отдельно. Закрывает #98.

## 0.42.1 — 2026-10-31

- UI доски: карточки задач не вылезают из колонок (`min-w-0` / `overflow-hidden` / `break-words`); проекты в узкой панели — truncate + `title` с полным путём; сводка «В работе · Todo · Готово» как на мокапе. Закрывает #104.

## 0.42.0 — 2026-10-31

- Виджет macOS снова встраивается в бандл: `src-tauri/macos-widget/` (xcodegen → `.appex`), `beforeBundleCommand` + `bundle.macOS.files` → `Contents/PlugIns/`, ad-hoc `sign:macos` / `widget:install`, `tauri-plugin-widgets-api` + postinstall-патч Swift Package. Docs: `docs/widget-macos.md`. Закрывает #103 (разрыв со спайком: плагин был, `.appex` — нет).

## 0.41.0 — 2026-10-30

- UI на shadcn-svelte (Tailwind v4, bits-ui, paneforge): Button, Card, Table, Tabs, Badge, Input, Textarea, ToggleGroup, ScrollArea, Resizable; тема в палитре мозаики. Колонки — с перетаскиваемыми границами. Панели портов, задач и ревью агентов переведены на компоненты. Исходники компонентов взяты с GitHub (shadcn-svelte@1.0.0): реестр сайта недоступен из песочницы, поэтому `components.json` написан вручную.

## 0.40.0 — 2026-10-30

- Имя пользователя для озвучки: `alerts.voice.user_name` и `user_pronunciation`, плейсхолдер `{user}` в фразах по умолчанию (по умолчанию «Олег»). UI-поля пока нет. Закрывает #99.

## 0.39.0 — 2026-10-30

- UI: тема в палитре мозаики (`references/mockups`), переключатель вида «Колонки / Вкладки» (режим в `localStorage`). Режим меню-бар и хранение в `settings.json` — позже.

## 0.38.1 — 2026-10-30

- Макеты интерфейса в `references/mockups/`: колонки, меню-бар, вкладки; стиль из мозаики `references/media`; исходники HTML. Закрывает #93.

## 0.38.0 — 2026-10-30

- Запасной Swift WidgetKit: scaffold `macos-widgetkit/` (Timeline + App Group + чтение `summary.json`), модуль `widgetkit_fallback`, `settings.widgetkit.enabled`, команда `sync_summary_to_widgetkit`; при ошибке `tauri-plugin-widgets` — forced запись в App Group. Docs `docs/widgetkit-fallback.md`. Не App Store / не iOS extension. Закрывает #38.

## 0.37.0 — 2026-10-30

- Синхронизация `summary.json` в iCloud Drive для iPhone: `settings.icloud` (`enabled`, `folder`), модуль `icloud_sync`, атомарная запись в `CloudDocs/<folder>/summary.json`, команда `sync_summary_to_icloud` (best-effort из `refresh_widget_from_summary`). Docs `docs/icloud-summary.md`. Не WidgetKit (#38). Закрывает #37.

## 0.36.0 — 2026-10-30

- Адаптер Notion `NotionSource`: REST через `curl` + `NOTION_TOKEN`, query database, маппинг title/status/select/checkbox, кэш 60 с, пагинация. Поля `database_id` / `title_property` / `status_property` в схеме; docs в `tasks-format.md`. При старте агента remote page не меняется. Закрывает #36.

## 0.35.1 — 2026-10-30

- README: что это, запуск, `orchestrator.json`, ссылки на docs. Указатель `docs/README.md` (ADR, схемы, форматы, фазы). Закрывает #35.

## 0.35.0 — 2026-10-30

- Опциональный раннер Ollama (`ollama run qwen2.5-coder:7b`) рядом с Claude CLI: выбор в `agents.runner` / селект на доске задач, модель в `agents.ollama_model`, тот же worktree + `collect_review`. Claude остаётся путём по умолчанию. Документ `docs/agents.md`. Закрывает #30.

## 0.34.1 — 2026-10-30

- Сканер: `MAX_DEPTH` 6→3, nesting подпроектов ≤1, расширен ignore-лист (`vendor`, `coverage`, `venv`, …). «Обновить задачи» не обходит глубокие nested `package.json`. Реальные top-level + один уровень (dashboard / parsers/news) сохраняются. Закрывает #85.

## 0.34.0 — 2026-10-30

- Запуск агента из задачи: кнопка «Исправить» на доске → `in_progress` (TODO.md / tasks.json; GitHub без remote close), worktree + limiter + Claude + `collect_review` в `AgentRunRegistry` для UI ревью. Accept задачу не закрывает. Документ `docs/agents.md`. Закрывает #33.

## 0.33.0 — 2026-10-30

- UI ревью результата агента: список runs со статусами, просмотр diff и лога тестов; «принять» показывает команду ручного `git merge` (без авто-merge), «отклонить» удаляет worktree. `AgentRunRegistry` + Tauri-команды, документ `docs/agents.md`. Закрывает #32.

## 0.32.1 — 2026-10-30

- `FileSource`: отсутствующие `TODO.md` / `tasks.json` дают `Ok([])` вместо стены `os error 2` при «Обновить задачи». Реальные IO/parse-ошибки по-прежнему `Err`.

## 0.32.0 — 2026-10-30

- После агента: прогон `commands.test` в worktree, сбор exit/лога и `git diff` к базовой ветке; статусы run `queued`…`accepted`, переход в `needs_review` / `failed`. Документ `docs/agents.md`. Закрывает #31.

## 0.31.0 — 2026-10-30

- Лимит параллельных агентов и проверка памяти перед стартом: `AgentLimiter` (по умолчанию 1 слот), `vm_stat` → доступная RAM, очередь при нехватке; запас `ollama_reserve_mb` если процесс `ollama` запущен. Конфиг `settings.json` → `agents`, доки в `docs/agents.md`. Закрывает #29.

## 0.30.0 — 2026-10-30

- Headless-раннер Claude Code CLI поверх worktree: `claude -p` в каталоге run, промпт из задачи + `CLAUDE.md`, сбор stdout/stderr/кода/длительности, таймаут и отмена SIGTERM дереву процессов (`DEMERZEL_CLAUDE_BIN`). Закрывает #28.

## 0.29.0 — 2026-10-30

- Менеджер git worktree для агентов: `create_worktree` / `remove_worktree` в `~/.orchestrator/worktrees/<project>/<run-id>` с веткой `agent/<run-id>` (`DEMERZEL_WORKTREES_DIR`), изоляция от основной ветки, документ `docs/agents.md`. Закрывает #27.

## 0.28.0 — 2026-10-30

- Голосовые алерты через macOS `say -v Milena`: короткие настраиваемые фразы (`alerts.voice`), вызов только при `DeliveryDecision.speak` (critical; тихие часы → off; ≤1/мин). Модуль `backend/speak`, трейт `VoiceSpeaker` для моков, `ingest_alert_event` → `spoken`. Закрывает #23.

## 0.27.0 — 2026-10-30

- Системные уведомления алертов через `tauri-plugin-notification`: короткий текст, показ только при `DeliveryDecision.notify` (тихие часы → только `critical`), клик фокусирует окно и открывает проект (`alerts:open-project`). Модуль `backend/notify`, команда `ingest_alert_event`, ADR Focus Mode (#22). Голос `say` не входит (#23).

## 0.26.0 — 2026-10-30

- Политика доставки алертов: дедуп одинаковых событий в окне 10 мин, троттлинг голоса ≤1/мин, тихие часы 23:00–08:00 `Europe/Berlin` (голос выкл., notify только `critical`). `DeliveryGate` / `DeliveryDecision` поверх `AlertCandidate`, поля в `alerts` settings, доки и `bun:test`. Закрывает #21.

## 0.25.0 — 2026-10-30

- Движок правил алертов: модель `AlertRule` / `AlertEvent` / `AlertCandidate`, уровни `critical` / `important` / `info`, правила по умолчанию и оценка в `settings.json` → `alerts`. Кандидаты без доставки (дедуп #21, notify #22, say #23). Документ `docs/alerts.md`, wireframe `references/alerts-rules-wireframe.md`, зеркало хелперов + `bun:test`. Закрывает #20.

## 0.24.0 — 2026-10-30

- Контракт `commands.migrate` в `orchestrator.json`: `dem sync` вызывает команду из конфига (ORM-агностично); без поля / пусто — `no commands.migrate; skipped`; результат в ответе sync и в логе `watch` как `[name migrate]`. Хелпер `resolve_migrate_command`, доки в `docs/cli.md`, описание в JSON Schema. Закрывает #43.

## 0.23.0 — 2026-10-30

- Рантайм Docker Compose для `type: docker-compose`: `dem up`/`down`/`status`/`sync` через `docker compose` (`ps --format json`, `up -d`, `stop`). Учёт контейнерного `Health`, опциональный YAML `healthcheck` после up/sync, понятная ошибка если Docker не запущен. Модуль `docker.rs`, тесты parse/ready, доки в `demerzel-yaml.md` / `cli.md`. Закрывает #42.

## 0.22.0 — 2026-10-30

- Схема и валидация корневого манифеста `demerzel.yaml`: сервисы `path` / `cmd` / `port` / `healthcheck`, `type: docker-compose` с `file`/`service`; неизвестные поля (`mode`, `notifications`) отклоняются. Документы `docs/demerzel-yaml.md` и `docs/demerzel.schema.json`, fallback на `orchestrator.json`, unit-тесты `load`/`parse`. Закрывает #41.

## 0.21.0 — 2026-10-30

- CLI `dem` / `demerzel`: `status`, `up`, `down`, `sync`, `watch` через Unix-socket supervisor (процессы, логи/метрики, migrate). Документ `docs/cli.md`, `DEMERZEL_RUNTIME_DIR`, unit-тесты entrypoint. `status`/`watch` поднимают supervisor без предварительного `up`. Закрывает #40.

## 0.20.0 — 2026-10-30

- UI доска задач: колонки по статусам (`todo` / `in_progress` / `done` / `blocked`), фильтр по проекту, агрегация через `project_tasks`, открытие issue URL или файла источника. Панель `TasksPanel`, хелперы фильтра/колонок/`canOpenTask` и `bun:test`. По wireframe `references/ports-tasks-wireframe.md`. Закрывает #19.

## 0.19.0 — 2026-10-30

- Адаптер GitHub Issues `GithubSource`: REST через `gh api`, фильтр по метке, исключение PR, кэш ответа по ETag/`If-None-Match` и минимум 60 с между запросами. Документ `docs/tasks-format.md` (секция GitHub), поля `repository`/`label` в схеме. Закрывает #18.

## 0.18.0 — 2026-10-30

- Файловый адаптер `FileSource`: парсер чекбоксов `TODO.md` (`parse_markdown`, статус `in_progress` через `- [~]`) и чтение `tasks.json` с квалификацией id по проекту. Документ `docs/tasks-format.md`, тесты на фикстурах. Закрывает #17.

## 0.17.0 — 2026-10-30

- Нормализованная модель задачи `Task { id, project, title, status, source, url? }` и статусы `todo` / `in_progress` / `done` / `blocked`. Трейт `TaskSource::fetch(project)`, сводка `count_by_status`, зеркало хелперов и `bun:test` на фронте. Закрывает #16.

## 0.16.0 — 2026-10-30

- UI таблица портов: колонки порт / проект / процесс / pid / статус (`ok`, конфликт, чужой), подсветка `conflict` и `foreign`, подтверждение перед остановкой только для процессов оркестратора (`managed_id`). Панель вынесена в `PortsPanel`, опрос через `list_ports`. Закрывает #15.

## 0.15.0 — 2026-10-30

- Фактические слушающие порты через `lsof`: парсер `-Fpcn`, инвентарь с метками `ok` / `foreign` / `conflict`, сопоставление pid через process group. При выделении порта учитываются и аренды, и сокеты ОС. Команда Tauri `list_ports`. Закрывает #14.

## 0.14.0 — 2026-10-30

- Реестр портов: диапазоны из `orchestrator.json`, проверка пересечений между проектами, аренда порта при старте процесса и передача в `PORT`. Закрывает #13.

## 0.13.0 — 2026-10-30

- Права доступа macOS: при `PermissionDenied` приложение и `dem status` показывают путь и инструкцию. Документ `docs/permissions.md`. Закрывает #12.

## 0.12.0 — 2026-10-30

- Настройки: корневые папки сканирования сохраняются в `settings.json` в папке данных приложения, при запуске сканируются автоматически. Пустые строки и дубликаты убираются. Команды `get_settings`, `save_settings`. Закрывает #11.

## 0.11.0 — 2026-10-30

- Интерфейс: дерево проектов и подпроектов из заданных папок, кнопка запуска, таблица процессов со статусами и кнопкой остановки, панель логов с обновлением раз в секунду. Команды Tauri: `scan_projects`, `start_process`, `stop_process`, `list_processes`, `process_logs`. Закрывает #10.

## 0.10.0 — 2026-10-30

- CLI `dem` (и `demerzel`): команды `status` (дерево проектов и подпроектов с конфигами, портами и типами) и `help`. Команды `up`, `down`, `sync`, `watch` тогда ещё не были подключены (закрыто в 0.21.0 / #40).

## 0.9.0 — 2026-10-30

- Логи процессов: stdout и stderr читаются в отдельных потоках, кольцевой буфер на 5000 строк, батч в UI раз в 100 мс. Закрывает #9.

## 0.8.0 — 2026-10-30

- Менеджер процессов: запуск через `sh -c` в папке проекта, `PORT` и `env` из конфига, отдельная process group. Остановка по всей группе: TERM, затем KILL через таймаут. Статусы через чистую функцию переходов. Закрывает #8.

## 0.7.0 — 2026-10-30

- Хранилище SQLite: миграции через `user_version` в транзакциях, таблицы projects, processes, port_leases, tasks, agent_runs, alerts. Уникальность аренды порта. Закрывает #7.

## 0.6.0 — 2026-10-30

- Сканер проектов: находит проекты по маркерам (`package.json`, `.git`, `orchestrator.json`) и вложенные подпроекты. Игнорирует `node_modules`, `target`, скрытые папки, глубина до 6. Закрывает #6.

## 0.5.0 — 2026-10-30

- Конфиг `orchestrator.json`: парсинг и валидация (имя, тип проекта, команды, диапазон портов, источник задач, env). Понятные ошибки. JSON Schema в `docs/orchestrator.schema.json`. Закрывает #5.

## 0.4.0 — 2026-10-30

- Обновление виджета по событию: только при изменении содержимого и не чаще раза в 30 секунд. Таймера нет, чтобы не тратить бюджет WidgetKit. Команда `refresh_widget_from_summary`. Закрывает #26.

## 0.3.0 — 2026-10-30

- Виджеты из `Summary`: `small` (число упавших и полоса портов) и `medium` (проекты и задачи). Плагин `tauri-plugin-widgets` подключён в приложение, capability `widgets:default`, конфиг App Group. Закрывает #25.

## 0.2.2 — 2026-10-30

- Lockfile: `bun.lock` теперь в репозитории, а не в `.gitignore`. Без него `bun install --frozen-lockfile` в CI падает.
## 0.2.0 — 2026-10-30

- Экспорт `summary.json` (версия 1): статусы проектов, сумма упавших, конфликты портов, счётчики задач и агентов. Запись атомарная, без переписывания неизменённого файла. Закрывает #24.

## 0.1.5 — 2026-10-30

- Бэкенд: модули `scanner`, `ports`, `process`, `tasks`, `alerts`, `agents`, `storage`, `summary` без зависимости от Tauri. Чистая логика покрыта тестами (15). Закрывает #4.

## 0.1.4 — 2026-10-30

- Доска GitHub Projects #13: все issues/PR синкаются со Status (workflow + `scripts/sync-project-board.mjs`). Конвенции обновлены.

## 0.1.3 — 2026-10-30

- Спайк виджета (#1): инструкция, конфиг виджета `widget.json`, критерии go/no-go в `spikes/1-widget-plugin/`.

## 0.1.2 — 2026-10-30

- Инструменты: Bun вместо pnpm (`bun.lock`, `beforeDevCommand`/`beforeBuildCommand`, CI через `oven-sh/setup-bun`).
- Конвенции: Bun и шаг обновления ссылки на удалённую ветку после push.

## 0.1.1 — 2026-10-30

- CI: GitHub Actions. Frontend на Ubuntu (`pnpm check`, `pnpm build`), backend на macOS (`cargo test`, `cargo clippy -D warnings`, `pnpm tauri build --debug`). Закрывает #34.

## 0.1.0 — 2026-10-30

- Скелет приложения: Tauri 2 + SvelteKit в режиме SPA (PR, закрывает #3).
- Переименование пакета и продукта в Demerzel, `ssr = false`, `adapter-static`.
- `.gitignore` для Node, Rust и сборки.

## 0.0.1 — 2026-10-30

- Переименование проекта в Demerzel (PR #docs-conventions).
- Метки для issues: фазы и области.
- Конвенции: ветки, коммиты, версии, журналы.
- Ассеты перенесены в `references/media/`, добавлены `references/README.md` и каркас `wireframes/`, `mockups/`.
