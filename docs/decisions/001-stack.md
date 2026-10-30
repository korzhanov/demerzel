# ADR-001: Стек и виджет macOS (`tauri-plugin-widgets`)

- **Status:** accepted (spike)
- **Date:** 2026-10-30
- **Issues:** #2, spike → #25–26 / #38
- **Report:** [001-report.md](./001-report.md) — как повторить, исправления, чеклист для агентов

## Context

Нужен виджет на macOS с обновлением состояния из Rust (без перезапуска приложения). Кандидат: `tauri-plugin-widgets`. Альтернатива при no-go: нативный Swift WidgetKit (#38).

## Decision

**Go** для `tauri-plugin-widgets` на macOS при локальной ad-hoc сборке, с оговорками ниже.

Плагин остаётся в стеке Demerzel. Переход на чистый Swift WidgetKit (#38) не требуется по результатам спайка, пока нет Team ID / App Groups в Developer portal.

## Evidence (spike `spikes/1-widget-plugin`)

| Проверка | Результат |
|---|---|
| Превью JSON в браузере | OK (`bunx tauri-plugin-widgets-api preview`) |
| Сборка `.appex` + embed в `Contents/PlugIns/` | OK |
| Запись конфига из Rust (`set_widget_config`) | OK, раз в 10 с (`tick`) |
| `reload_all_timelines` из Rust | OK (trace: десятки reload) |
| Подпись / App Group без Team ID | OK через `transport: widgetContainer` |

### Версии

- macOS 27.2 (Build 26B123), Darwin arm64
- Xcode 27.2
- Rust 1.94.1
- Bun 1.4.3 (packageManager заявлен 1.4.4)
- `tauri-plugin-widgets` / `tauri-plugin-widgets-api` **0.6.2**
- App Group: `group.app.demerzel.orchestrator`
- Transport: `widgetContainer` (ad-hoc, Team ID отсутствует)

### Что ломалось

1. **CLI:** пакет `tauri-widgets` в npm нет (404). Бинарь `tauri-widgets` поставляется в `tauri-plugin-widgets-api` → `bunx tauri-plugin-widgets-api …` или локальный `bunx tauri-widgets` после `bun add`.
2. **Swift Package:** в npm-артефакте `Package.swift` ссылается на отсутствующий `Tests/TauriWidgetsTests` → `xcodebuild` падает. В спайке патч: `scripts/TauriWidgets.Package.swift` + `postinstall`.
3. **`IO error: Operation not permitted (os error 1)`:** debug-бандл Tauri оставлял host только linker-signed. LaunchServices (`open`/Finder) тогда получает EPERM на запись в `~/Library/Containers/<appex>/Data/widget_data.json`. Лечение: ad-hoc `codesign` host **без** App Sandbox (`scripts/sign-macos.sh` / `bun run spike:install`) после того, как контейнер extension уже создан (один запуск установленного `.app`).
4. **DMG:** `hdiutil detach` иногда «ресурс занят» — на `.app` не влияет.
5. **Doctor:** `no render events` до ручного добавления виджета в Notification Center / на рабочий стол — ожидаемо; data+reload path из Rust подтверждён.

## Consequences

- В основном приложении: `tauri add widgets`, `init-macos`, App Group `group.app.demerzel.orchestrator`, post-sign для локальных сборок без Team ID.
- Для TestFlight / MAS нужен Team ID и `transport: appGroup`.
- Следить за дневным бюджетом WidgetKit reload (~40–70/сутки); для демо-тика 10 с — только debug.
- Завести upstream-issue на s00d/tauri-plugin-widgets про missing Tests path в npm package.

## Alternatives considered

- **Swift WidgetKit (#38)** — отложен; использовать, если ad-hoc/plugin окажется неприемлемым для релиза или App Groups не заведутся с Team ID.
