# Swift WidgetKit как запасной вариант (#38)

Scaffold нативного Widget Extension для macOS. **Основной** путь виджета — `tauri-plugin-widgets` ([ADR-001](decisions/001-stack.md)). Native WidgetKit нужен, если плагин no-go для релиза, App Groups с Team ID, или плагин перестал покрывать нужное.

Не входит: App Store / TestFlight, отдельный iOS Widget Extension (для iPhone — [icloud-summary.md](icloud-summary.md)).

## Зачем

| Путь | Кто рисует | Откуда данные |
|---|---|---|
| Primary | `tauri-plugin-widgets` IR | `set_widget_config` + reload timelines |
| Fallback (#38) | Swift `DemerzelNativeWidget` | `summary.json` в App Group |

Оба используют App Group `group.app.demerzel.orchestrator` и тот же format `summary.json` (#24).

## Путь к данным

| | |
|---|---|
| App Group | `group.app.demerzel.orchestrator` |
| Файл | `~/Library/Group Containers/group.app.demerzel.orchestrator/summary.json` |
| Kind | `DemerzelNativeWidget` |
| Bundle id extension | `app.demerzel.orchestrator.widgetkit` |

Формат — versioned JSON (`version`, `projects`, `failing_total`, порты/задачи/агенты, `updated_at`). Запись атомарная (`write_if_changed`).

## Настройка (`settings.json`)

```json
{
  "widgetkit": {
    "enabled": true
  }
}
```

| Поле | По умолчанию | Смысл |
|---|---|---|
| `enabled` | `false` | Писать `summary.json` в App Group при каждом refresh (рядом с плагином) |

Если `tauri-plugin-widgets` падает при refresh, host **всегда** пытается forced-запись в App Group (даже при `enabled: false`) — это и есть runtime fallback.

## Команды Tauri

| Команда | Поведение |
|---|---|
| `sync_summary_to_widgetkit(summary)` | Sync по `settings.widgetkit`; `SkippedDisabled` / `Unchanged` / `Written` |
| `refresh_widget_from_summary` | iCloud best-effort → plugin; при ошибке плагина → forced App Group write |

## Scaffold

Каталог [`macos-widgetkit/`](../macos-widgetkit/):

- `Sources/DemerzelWidget.swift` — TimelineProvider + UI (small/medium), чтение App Group
- `project.yml` — xcodegen → `.xcodeproj`
- `DemerzelWidget.entitlements` — App Group
- `Info.plist` — WidgetKit extension point

Сборка локально:

```sh
cd macos-widgetkit
xcodegen generate
# открыть в Xcode, выставить Team, подписать, embed .appex в Demerzel.app
```

Без Team ID контейнер App Group на устройстве может не создаться — см. ADR-001 (`transport: widgetContainer` для плагина). Для production fallback нужен Developer portal App Group.

## Модуль

Чистая логика: `src-tauri/src/backend/widgetkit_fallback.rs` (без Tauri в unit-тестах). Зеркало путей: `src/lib/widgetkit.ts`.
