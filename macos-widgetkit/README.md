# Native WidgetKit fallback (#38)

Запасной виджет macOS на чистом Swift WidgetKit: читает `summary.json` из App Group
`group.app.demerzel.orchestrator`. Основной путь — `tauri-plugin-widgets` ([ADR-001](../docs/decisions/001-stack.md)).

Полная инструкция: [docs/widgetkit-fallback.md](../docs/widgetkit-fallback.md).

## Быстрый старт

```sh
# из корня репо
brew install xcodegen   # если ещё нет
cd macos-widgetkit
xcodegen generate
open DemerzelWidgetKit.xcodeproj
```

В Xcode: выставить Team ID, подписать extension, встроить `.appex` в host `Demerzel.app`
(`Contents/PlugIns/`). В `settings.json` приложения:

```json
{ "widgetkit": { "enabled": true } }
```

Host пишет `~/Library/Group Containers/group.app.demerzel.orchestrator/summary.json`.

## Что не входит в scope

- Публикация в App Store / TestFlight
- Отдельный iOS target (для iPhone — iCloud Files, #37)
- Замена `tauri-plugin-widgets` по умолчанию
