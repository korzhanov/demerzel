# Спайк 1: tauri-plugin-widgets на macOS

Цель: подтвердить, что плагин выводит виджет на macOS и умеет обновлять его из Rust по событию. Это решает, остаётся ли плагин или переходим на Swift WidgetKit (#38).

Требуется: macOS, Xcode, Bun 1.4.4, Rust stable, `xcodegen` (`brew install xcodegen`).

## Шаги

1. Превью без сборки (CLI живёт в `tauri-plugin-widgets-api`, не в `tauri-widgets`):
   ```sh
   bunx tauri-plugin-widgets-api preview ./widget.json --size small --watch
   ```
   Проверить: виджет рисуется в браузере, правка JSON перезагружает превью.

2. Минимальное приложение (каталог `spike-app/`, основной проект не трогаем):
   ```sh
   bunx create-tauri-app spike-app --template svelte-ts --manager bun --yes
   cd spike-app
   bun install
   bun run tauri add widgets
   bun add tauri-plugin-widgets-api
   bunx tauri-widgets init
   bunx tauri-widgets signing --apply
   ```
   Группа App Group: `group.app.demerzel.orchestrator`.

   Если `xcodebuild` ругается на `Tests/TauriWidgetsTests` — применить патч
   `scripts/patch-swift-package.mjs` (уже в `spike-app` через `postinstall`).

3. Регистрация в Rust: `tauri_plugin_widgets::init()` в `lib.rs`, `widgets:default` в `capabilities`.
   Тик раз в 10 с + команда `tick` пишут конфиг через `set_widget_config` и `reload_all_timelines`.

4. Сборка и установка:
   ```sh
   bun run tauri build --debug
   bun run spike:install   # codesign ad-hoc → /Applications → open
   ```
   Без `scripts/sign-macos.sh` host часто остаётся linker-signed → при запуске через
   Finder/`open` запись в контейнер виджета даёт `IO error: Operation not permitted`.

5. Notification Center → Edit Widgets → добавить **Demerzel**. Значение «N failing»
   должно меняться без перезапуска приложения.

## Критерии

- **go:** виджет отображается, значение меняется после `tick`, без перезапуска.
- **no-go:** виджет не добавляется, подпись/App Group не настраиваются, либо обновление возможно только из JS по нажатию. Тогда #25–26 переезжают на Swift (#38).

## Результат

- ADR: [`docs/decisions/001-stack.md`](../../docs/decisions/001-stack.md) — **go**
- Полный отчёт (грабли, как повторить): [`docs/decisions/001-report.md`](../../docs/decisions/001-report.md)
