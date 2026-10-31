# Виджет macOS (`tauri-plugin-widgets`)

Основной путь виджета — `tauri-plugin-widgets` ([ADR-001](decisions/001-stack.md)).  
Запасной Swift WidgetKit: [widgetkit-fallback.md](widgetkit-fallback.md).

## Почему виджета не было в списке (#103)

В main были `plugins.widgets` и `tauri_plugin_widgets::init()`, но **не** собирался и не встраивался `.appex`. WidgetKit показывает приложение в Edit Widgets только если в бандле есть extension:

`Demerzel.app/Contents/PlugIns/TauriWidgetExtension.appex`

Спайк уже делал это через `beforeBundleCommand` + `bundle.macOS.files`. Тот же sidecar перенесён в `src-tauri/macos-widget/`.

## Сборка и установка (локально, ad-hoc)

Требуется: Xcode, `xcodegen` (`brew install xcodegen`), Bun.

```sh
bun install                          # postinstall патчит Swift Package (нет Tests/)
bun run tauri build --debug          # beforeBundleCommand → build-widget.sh → embed PlugIns
bun run widget:install               # ad-hoc sign → /Applications → open
# или кнопка «установить виджеты» в UI (install_widgets)
```

`bundle.targets` = `["app"]` (без DMG): для виджетов нужен `.app` в `target/.../bundle/macos/`. DMG часто падает на `hdiutil … Ресурс занят` из‑за залипшего interstitial volume — для релиза при необходимости временно поставь `"all"` или `["app","dmg"]` и перед сборкой сделай `diskutil eject` залипших `/Volumes/dmg.*`.

Или по шагам:

```sh
bun run tauri build --debug
# .app: src-tauri/target/debug/bundle/macos/Demerzel.app
ls "src-tauri/target/debug/bundle/macos/Demerzel.app/Contents/PlugIns/TauriWidgetExtension.appex"
bun run sign:macos
# установить в /Applications (WidgetKit плохо подхватывает из target/)
rm -rf /Applications/Demerzel.app
cp -R src-tauri/target/debug/bundle/macos/Demerzel.app /Applications/
bun run sign:macos /Applications/Demerzel.app
open /Applications/Demerzel.app
```

Без `sign:macos` host часто остаётся linker-signed → EPERM при записи в контейнер виджета (см. [001-report.md](decisions/001-report.md)).

## Проверка в Widget Gallery

1. Запустить установленное `/Applications/Demerzel.app` хотя бы раз (containermanagerd создаёт контейнер appex).
2. Notification Center → **Edit Widgets** (или правый клик по рабочему столу → Edit Widgets).
3. Найти **Demerzel** → Add Widget (small/medium/large).
4. Обновить данные в приложении (скан портов / refresh summary) — виджет должен обновиться без перезапуска app.

Если виджета нет в списке:

```sh
# есть ли PlugIns?
ls /Applications/Demerzel.app/Contents/PlugIns/
# подпись не linker-signed?
codesign -dv --verbose=2 /Applications/Demerzel.app 2>&1 | head -20
# doctor плагина
bunx tauri-widgets doctor
```

Перезагрузка галереи: убрать виджет → Quit Demerzel → снова open → Edit Widgets. Иногда помогает `killall WidgetKitExtensionHost` (macOS перезапустит host).

Данные (transport `widgetContainer`):

`~/Library/Containers/app.demerzel.orchestrator.widgetkit/Data/widget_data.json`

Превью IR без сборки:

```sh
bun run widget:preview
```

## Конфиг

| | |
|---|---|
| App Group | `group.app.demerzel.orchestrator` |
| Host bundle | `app.demerzel.orchestrator` |
| Extension | `app.demerzel.orchestrator.widgetkit` |
| Transport | `widgetContainer` (ad-hoc без Team ID) |
| Widget id | `default` |
| Display name | Demerzel |

С Team ID / MAS: `transport: appGroup` и Developer portal App Group — см. ADR-001.

## Пульс проектов (#98 / #97)

В `summary.json` у проекта может быть опциональный снимок пульса. Источник — `orchestrator.json` → `pulse` (команда или файл, опрос с таймаутом): [pulse.md](pulse.md) (#97).

```json
{
  "name": "parser",
  "running": 1,
  "failing": 0,
  "pulse": {
    "summary": "1500 новых лемм, 450 проверено",
    "metrics": { "parsed": 1500, "checked": 450 },
    "eta_seconds": 600,
    "state": "running"
  }
}
```

Виджет:

| Размер | Что показывает |
|---|---|
| small | failing + порты + **1** строка пульса (`name: summary, ещё N мин`) |
| medium | проекты / задачи + до **3** строк пульса |

Метрики в IR не рисуются. IR собирается в `widget::config` (small+medium). `refresh_widget` сравнивает текст IR и вызывает `reload_all_timelines` только при изменении (throttle ≥30 с) — бюджет WidgetKit не жжётся на тот же пульс.

Превью IR: `bun run widget:preview` (см. `widget.json`). Пример parser: `examples/parser/`.
