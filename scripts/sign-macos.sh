#!/usr/bin/env bash
# Ad-hoc sign the bundled .app so widgetContainer writes into the extension
# sandbox are allowed when launched via LaunchServices (open/Finder).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENTS="$ROOT/src-tauri/macos-widget/App.entitlements"
APP="${1:-$ROOT/src-tauri/target/debug/bundle/macos/Demerzel.app}"

if [[ ! -d "$APP" ]]; then
  echo "missing app bundle: $APP" >&2
  exit 1
fi

IDENTITY="${WIDGET_SIGN_IDENTITY:-${APPLE_SIGNING_IDENTITY:--}}"

echo "[sign] $APP (identity=$IDENTITY)"
codesign --force --sign "$IDENTITY" --entitlements "$ENTS" "$APP/Contents/MacOS/"*
# Re-sign appex first (nested), then the bundle.
if [[ -d "$APP/Contents/PlugIns/TauriWidgetExtension.appex" ]]; then
  find "$APP/Contents/PlugIns/TauriWidgetExtension.appex/Contents" -type f -name '*.dylib' \
    -exec codesign --force --sign "$IDENTITY" {} \;
  codesign --force --sign "$IDENTITY" \
    --entitlements "$ROOT/src-tauri/macos-widget/TauriWidgetExtension.entitlements" \
    "$APP/Contents/PlugIns/TauriWidgetExtension.appex"
fi
codesign --force --sign "$IDENTITY" --entitlements "$ENTS" "$APP"
codesign -dv --verbose=2 "$APP" 2>&1 | head -15
echo "[sign] done"
