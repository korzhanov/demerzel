#!/usr/bin/env bash
# Сборка debug-бандла и установка в /Applications вместе с виджетом.
# Сборка → подпись → замена /Applications/Demerzel.app → запуск.
# Только установка без сборки: bash scripts/build-and-install.sh --install-only
# Сборка без установки: NO_INSTALL=1 bun run build:install
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

APP_SRC="$ROOT/src-tauri/target/debug/bundle/macos/Demerzel.app"
APP_DST="/Applications/Demerzel.app"

if [[ "${1:-}" != "--install-only" ]]; then
  # Только .app: DMG для установки не нужен, а hdiutil часто не может отмонтировать образ.
  # Без подписи обновлений: приватного ключа updater'а на Mac нет.
  bun run tauri build --debug --bundles app --config '{"bundle":{"createUpdaterArtifacts":false}}'
fi

if [[ "${NO_INSTALL:-}" == "1" ]]; then
  echo "[install] пропущено (NO_INSTALL=1). Бандл: $APP_SRC"
  exit 0
fi

if [[ ! -d "$APP_SRC/Contents/PlugIns/TauriWidgetExtension.appex" ]]; then
  echo "В бандле нет TauriWidgetExtension.appex, виджеты не установятся: $APP_SRC" >&2
  echo "Проверьте xcodegen (brew install xcodegen) и лог сборки виджета." >&2
  exit 1
fi

# Работающую копию закрываем, иначе её файлы не заменить.
osascript -e 'tell application "Demerzel" to quit' >/dev/null 2>&1 || true
for _ in $(seq 1 20); do
  pgrep -xq Demerzel || break
  sleep 0.5
done

bash scripts/sign-macos.sh "$APP_SRC"
rm -rf "$APP_DST"
cp -R "$APP_SRC" /Applications/
bash scripts/sign-macos.sh "$APP_DST"
open "$APP_DST"
echo "[install] установлено: $APP_DST. Виджеты: Notification Center → Edit Widgets → Demerzel."
