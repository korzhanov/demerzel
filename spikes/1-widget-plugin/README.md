# Спайк 1: tauri-plugin-widgets на macOS

Цель: подтвердить, что плагин выводит виджет на macOS и умеет обновлять его из Rust по событию. Это решает, остаётся ли плагин или переходим на Swift WidgetKit (#38).

Требуется: macOS, Xcode, Bun 1.4.4, Rust stable.

## Шаги

1. Превью без сборки:
   ```sh
   bunx tauri-widgets preview ./widget.json --size small --watch
   ```
   Проверить: виджет рисуется в браузере, правка JSON перезагружает превью.

2. Минимальное приложение (отдельный каталог, чтобы не трогать основной проект):
   ```sh
   bunx create-tauri-app spike-app --template svelte-ts --manager bun --yes
   cd spike-app
   bun run tauri add widgets   # или см. README плагина
   bunx tauri-widgets init
   bunx tauri-widgets signing --apply
   ```
   Группа App Group: `group.app.demerzel.orchestrator`.

3. Регистрация в Rust: `tauri_plugin_widgets::init()` в `lib.rs`, `widgets:default` в `capabilities`.

4. Установка виджета: `bun run tauri build --debug`, запустить `.app`, открыть Notification Center, добавить виджет Demerzel.

5. Обновление из Rust по событию: команда `tick` раз в 10 секунд меняет значение «failing» и вызывает обновление виджета через API плагина. Результат: значение в виджете меняется без перезапуска приложения.

## Критерии

- **go:** виджет отображается, значение меняется после `tick`, без перезапуска.
- **no-go:** виджет не добавляется, подпись/App Group не настраиваются, либо обновление возможно только из JS по нажатию. Тогда #25–26 переезжают на Swift (#38).

## Результат

Записать в `docs/decisions/001-stack.md` (ADR-001, #2): go/no-go, версии плагина и macOS, что сломалось, если сломалось.
