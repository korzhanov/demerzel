# Report 001: Спайк `tauri-plugin-widgets` на macOS

- **Дата:** 2026-10-30
- **Ветка:** `spike/1-widget-plugin`
- **ADR:** [001-stack.md](./001-stack.md) — вердикт **go**
- **Код спайка:** `spikes/1-widget-plugin/` (изолированный `spike-app/`, основной Demerzel не трогали)
- **Аудитория:** люди и агенты, которым нужно повторить проверку или перенести выводы в основное приложение

---

## 1. Цель

Подтвердить, что `tauri-plugin-widgets`:

1. рисует macOS WidgetKit-виджет из JSON/IR;
2. умеет **обновлять** его из **Rust** по таймеру/команде без перезапуска приложения.

От этого зависит: оставляем плагин в стеке или уходим на Swift WidgetKit (#38).

---

## 2. Вердикт (кратко)

| Критерий | Статус |
|---|---|
| Превью JSON в браузере | OK |
| Сборка `.appex` + embed в `.app` | OK (с патчем Swift Package) |
| Обновление из Rust (`set_widget_config` + reload) | OK, каждые ~10 с |
| Ad-hoc без Team ID | OK через `transport: widgetContainer` |
| Запуск через Finder/`open` без EPERM | OK **только после** `scripts/sign-macos.sh` |
| Визуально в Notification Center | нужно добавить виджет вручную (doctor: нет render events, пока не добавлен) |

**Решение:** плагин остаётся. Перенос #25–26 на Swift (#38) не требуется по этому спайку.

---

## 3. Окружение, на котором прогоняли

| Компонент | Версия / значение |
|---|---|
| macOS | 27.2 (Build 26B123), arm64 |
| Xcode | 27.2 |
| Rust | 1.94.1 stable |
| Bun | 1.4.3 (в `packageManager` указан 1.4.4) |
| `xcodegen` | `/opt/homebrew/bin/xcodegen` |
| Плагин (Cargo + npm) | `tauri-plugin-widgets` / `tauri-plugin-widgets-api` **0.6.2** |
| App Group | `group.app.demerzel.orchestrator` |
| Bundle ID host | `app.demerzel.orchestrator` |
| Bundle ID appex | `app.demerzel.orchestrator.widgetkit` |
| Transport | `widgetContainer` (нет Team ID / codesign identity) |
| Widget id (Swift + Rust) | `default` |

Codesign identities: **0 valid** → только ad-hoc (`-`).

---

## 4. Что сделано (артефакты)

```
spikes/1-widget-plugin/
  README.md                 # краткий чеклист спайка
  widget.json               # IR для превью
  spike-app/                # create-tauri-app (svelte-ts + bun)
    scripts/
      patch-swift-package.mjs      # postinstall: чинит Package.swift
      TauriWidgets.Package.swift   # эталон патча (без Tests/)
      sign-macos.sh                # ad-hoc codesign host+appex
    src-tauri/
      src/lib.rs                   # init widgets + tick каждые 10 с
      tauri.conf.json              # plugins.widgets, beforeBundleCommand, PlugIns
      macos-widget/                # init-macos sidecar (xcodegen → .appex)
      capabilities/default.json    # widgets:default
docs/decisions/
  001-stack.md              # ADR go/no-go
  001-report.md             # этот отчёт
```

### Поведение `spike-app`

- При старте: один `set_widget_config` (failing=0).
- Фон: каждые 10 с bump `0..9`, запись IR, `reload_all_timelines` (через `run_on_main_thread`).
- UI: кнопка `tick` → команда Rust `tick`.
- Данные на диске (ad-hoc):  
  `~/Library/Containers/app.demerzel.orchestrator.widgetkit/Data/widget_data.json`

---

## 5. Исправления и грабли (обязательно знать)

### 5.1 CLI: `bunx tauri-widgets` → 404

Пакет `tauri-widgets` в npm **не существует**. Бинарь `tauri-widgets` лежит в **`tauri-plugin-widgets-api`**.

```sh
bunx tauri-plugin-widgets-api preview ./widget.json --size small --watch
# после bun add tauri-plugin-widgets-api:
bunx tauri-widgets init
bunx tauri-widgets signing --apply
bunx tauri-widgets doctor
```

### 5.2 `tauri add tauri-plugin-widgets` ломает имя

CLI склеивает префикс → пытается `tauri-plugin-tauri-plugin-widgets`. Нужно:

```sh
bun run tauri add widgets
```

### 5.3 Swift Package: missing `Tests/TauriWidgetsTests`

В npm-пакете `Package.swift` указывает на каталог тестов, которого нет → `xcodebuild` exit 74.

**Патч в спайке:** `postinstall` копирует `scripts/TauriWidgets.Package.swift` поверх  
`node_modules/tauri-plugin-widgets-api/swift/Package.swift` (убирает `testTarget` и зависимость snapshot-testing).

Upstream: завести issue на s00d/tauri-plugin-widgets.

### 5.4 `IO error: Operation not permitted (os error 1)`

**Симптом:** после `open /Applications/Demerzel.app` (или Finder) push из Rust падает с EPERM на запись в контейнер appex.

**Причина:** debug-бандл Tauri часто оставляет host **linker-signed**, без нормального ad-hoc + entitlements. LaunchServices тогда режет запись в `~/Library/Containers/<appex>/…`.

**Лечение:**

1. Установить `.app` в `/Applications` (WidgetKit иначе плохо подхватывает).
2. Один раз запустить, чтобы containermanagerd создал контейнер appex.
3. Подписать host **без** `com.apple.security.app-sandbox` и re-sign appex:

```sh
cd spikes/1-widget-plugin/spike-app
bun run spike:install
# = sign → cp в /Applications → sign снова → open
```

Или вручную: `bash scripts/sign-macos.sh [/path/to/Demerzel.app]`.

Проверка подписи:

```sh
codesign -dv --verbose=2 /Applications/Demerzel.app
# Identifier=app.demerzel.orchestrator, Signature=adhoc, не linker-signed
```

### 5.5 DMG: `hdiutil … Ресурс занят`

На наличие `…/bundle/macos/Demerzel.app` не влияет. При необходимости: `hdiutil detach /dev/diskN -force`.

### 5.6 Бюджет WidgetKit

~40–70 reload/сутки. Тик раз в 10 с — **только для спайка/debug**. В проде — реже или native `{ type: "timer" }`.

### 5.7 NSUserDefaults warning

В логе: *Using your own bundle identifier as an NSUserDefaults suite name…* — шум при ad-hoc/widgetContainer, на запись `widget_data.json` не блокирует.

---

## 6. Как повторить с нуля (чеклист агента)

### Предусловия

```sh
sw_vers && rustc --version && bun --version && xcodebuild -version | head -2
which xcodegen || brew install xcodegen
security find-identity -v -p codesigning   # ожидаемо: 0 identities → widgetContainer
```

### A. Только превью IR (без Tauri)

```sh
cd spikes/1-widget-plugin
bunx tauri-plugin-widgets-api preview ./widget.json --size small --watch --open
```

### B. Уже собранный `spike-app` (быстрый путь)

```sh
cd spikes/1-widget-plugin/spike-app
bun install                          # применит patch-swift-package
bun run tauri build --debug          # .app в src-tauri/target/debug/bundle/macos/
bun run spike:install                # sign + /Applications + open
```

Дальше вручную:

1. Notification Center → Edit Widgets → **Demerzel**.
2. Смотреть «N failing» или файл данных:

```sh
python3 - <<'PY'
import json, os
p = os.path.expanduser(
  "~/Library/Containers/app.demerzel.orchestrator.widgetkit/Data/widget_data.json"
)
d = json.load(open(p))
cfg = json.loads(d["config:default"])
print([c.get("content") for c in cfg["small"]["children"] if c.get("type")=="text"])
print("updated_at", d.get("__meta_updated_at__"))
PY
# через ~10 с updated_at и N failing должны смениться
```

Doctor:

```sh
cd spikes/1-widget-plugin/spike-app && bunx tauri-widgets doctor
```

Ожидание: transport/entitlements OK; `no render events` — пока виджет не добавлен на экран.

### C. Создание `spike-app` заново (если каталога нет)

```sh
cd spikes/1-widget-plugin
bunx create-tauri-app@latest spike-app --template svelte-ts --manager bun --yes
cd spike-app
bun install
bun run tauri add widgets
bun add tauri-plugin-widgets-api

# скопировать/восстановить патчи из текущего спайка:
#   scripts/patch-swift-package.mjs
#   scripts/TauriWidgets.Package.swift
#   scripts/sign-macos.sh
# и postinstall / spike:install в package.json
bun scripts/patch-swift-package.mjs

bunx tauri-widgets init
# выставить в tauri.conf.json:
#   productName Demerzel
#   identifier app.demerzel.orchestrator
#   plugins.widgets.appGroup = group.app.demerzel.orchestrator
#   extensionBundleId = app.demerzel.orchestrator.widgetkit
#   transport = widgetContainer   # без Team ID
bunx tauri-widgets signing --apply

# lib.rs: tauri_plugin_widgets::init(), tick + фон 10 с
# capabilities: widgets:default
# MyWidget.swift: appGroup + configurationDisplayName "Demerzel", widgetId "default"

bun run tauri build --debug
bun run spike:install
```

Эталон реализации тика: `spike-app/src-tauri/src/lib.rs`.

---

## 7. Как проверить, что «go» всё ещё держится

1. Процесс жив: `pgrep -lf spike-app` или `Demerzel`.
2. Файл данных обновляется ≥1 раз / 10–15 с (см. скрипт выше).
3. Trace:  
   `~/Library/Application Support/app.demerzel.orchestrator/widgets/widget_trace.json`  
   — есть события reload.
4. После добавления виджета UI: число failing меняется **без** перезапуска `.app`.
5. `bunx tauri-widgets doctor` — 0 errors (warnings по ad-hoc/budget допустимы).

**No-go сигналы:** не собирается `.appex` без патча и патч нельзя поддерживать; EPERM неустраним даже после sign; reload только из JS по клику; виджет не появляется в галерее после install в `/Applications`.

---

## 8. Перенос в основное приложение Demerzel (заметки для следующих задач)

1. `bun run tauri add widgets` + `bun add tauri-plugin-widgets-api`.
2. `bunx tauri-widgets init` / `init-macos`, App Group **`group.app.demerzel.orchestrator`**.
3. Зафиксировать postinstall-патч Swift Package (или ждать upstream).
4. Post-bundle: аналог `sign-macos.sh` для локальных ad-hoc сборок; с Team ID → `signing --apply` и `transport: appGroup`.
5. Обновления статуса портов/агентов — из Rust (`WidgetExt`), не только из webview.
6. Не жечь WidgetKit budget частыми reload в release.

---

## 9. Полезные пути

| Что | Путь |
|---|---|
| Собранный `.app` | `spikes/1-widget-plugin/spike-app/src-tauri/target/debug/bundle/macos/Demerzel.app` |
| Установка | `/Applications/Demerzel.app` |
| Данные виджета (widgetContainer) | `~/Library/Containers/app.demerzel.orchestrator.widgetkit/Data/widget_data.json` |
| Trace | `~/Library/Application Support/app.demerzel.orchestrator/widgets/widget_trace.json` |
| Документация плагина | https://s00d.github.io/tauri-plugin-widgets |
| Rust update API | https://s00d.github.io/tauri-plugin-widgets/api/rust |
| macOS setup | https://s00d.github.io/tauri-plugin-widgets/guide/setup/macos |

---

## 10. Связанные issues / follow-up

- ADR / стек: #2 → [001-stack.md](./001-stack.md)
- Виджет в продукте: #25–26 (остаются на плагине)
- Запасной Swift: #38 (отложен)
- Upstream: missing `Tests/` в npm `tauri-plugin-widgets-api` Swift package
- Когда появится Apple Team ID: переключить `transport` на `appGroup`, убрать зависимость от ручного ad-hoc sign для записи данных
