# ADR-003: App Intents / Siri Shortcuts для Demerzel

- **Status:** accepted (spike)
- **Date:** 2026-10-31
- **Issues:** #100 (spike) → #101 (интенты статуса и перезапуска)
- **Report:** [003-report.md](./003-report.md)
- **Spike:** `spikes/2-siri-app-intents/`

## Context

Нужен App Shortcut «Спроси Demerzel …» на macOS (App Intents, 13+): параметр-сущность проект, ответ диалогом + сниппет. Вопросы: русская фраза, обязательное имя приложения в phrase, связь intent ↔ Tauri (App Group / Unix-сокет / CLI), sandbox. Альтернатива при no-go — только CLI / Shortcuts без нативных App Intents.

## Decision

**Go** для App Intents в Swift, встроенных в бандл Demerzel (host Tauri), с транспортом **Unix-сокет `dem.sock`** (#102).

| Тема | Решение |
|---|---|
| Фреймворк | App Intents + `AppShortcutsProvider` (Swift-only; Rust/Tauri intents не пишем) |
| Транспорт live-команд | `~/.orchestrator/runtime/dem.sock` — тот же JSON, что `dem pulse` / `dem restart` |
| Кэш списка проектов | Опционально App Group `group.app.demerzel.orchestrator` / `summary.json` (#38) для `EntityQuery` |
| CLI subprocess | Не primary: хрупкий PATH, хуже для sandbox |
| Русские фразы | Да; каждая phrase с `\(.applicationName)` («Demerzel») |
| Диалог + сниппет | `ProvidesDialog` + `ShowsSnippetIntent` |
| Scope #101 | `AskDemerzelStatus(project)` и `RestartProcess(project)` с подтверждением |

## Evidence (spike `spikes/2-siri-app-intents`)

| Проверка | Результат |
|---|---|
| Swift → `dem.sock` → pulse JSON | OK (`scripts/dem-sock-probe.swift`) |
| Текст диалога «parser: …, ещё 10 мин» | OK |
| App Intent API: entity + dialog + snippet + App Shortcut | OK (эталон в `Sources/`) |
| Phrase без имени приложения | Запрещено Apple — документировано |
| Русская локаль фраз | OK по контракту LocalizedStringResource |
| Sandbox / MAS | Оговорка: локальный ad-hoc без App Sandbox — go; MAS нужен entitlement / helper |

### Версии окружения

- macOS 27.2 (Build 26B123), Darwin arm64
- Xcode 27.2, Swift 6.4.2
- Bun 1.4.4, Demerzel CLI с `pulse` (#102 / 0.45.0)

## Consequences

- #101 реализует интенты в основном приложении (Swift рядом с host / extension), не отдельный продукт.
- Root экосистемы (`DEMERZEL_ROOT` / UserDefaults) нужно пробросить из настроек Tauri.
- Не блокируем #101 ожиданием Team ID / App Store sandbox; локальный путь готов.
- iOS-клиент / remote Siri вне scope.

## Alternatives considered

- **Только Shortcuts без App Intents** — пользователь сам собирает сценарий; нет zero-setup App Shortcut и сущности проекта из коробки.
- **App Group как единственный канал** — подходит для чтения `summary.json`, не для `restart` и live pulse без гонок.
- **HTTP localhost** — лишняя поверхность; сокет уже есть и без сети наружу.
