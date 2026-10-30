#!/bin/bash
set -euo pipefail

# ─── macOS App Intents Extension Builder (#101) ───────────────────────────────
# Called via tauri.conf.json → build.beforeBundleCommand (before widget).
# Builds DemerzelAppIntents.appex for bundle.macOS.files → Contents/PlugIns/.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CONFIGURATION="${1:-Release}"
DERIVED_DATA="$SCRIPT_DIR/build"
EXT_ENTS="$SCRIPT_DIR/DemerzelAppIntents.entitlements"
IDENTITY="${SIRI_SIGN_IDENTITY:-${APPLE_SIGNING_IDENTITY:--}}"
GROUP="group.app.demerzel.orchestrator"

CONF="$SCRIPT_DIR/../tauri.conf.json"
if [[ ! -f "$CONF" ]]; then
  CONF="$(cd "$SCRIPT_DIR/../.." && pwd)/src-tauri/tauri.conf.json"
fi

if [[ -f "$CONF" ]] && command -v node >/dev/null 2>&1; then
  GROUP="$(node -e "
    const fs = require('fs');
    const conf = JSON.parse(fs.readFileSync(process.argv[1], 'utf8'));
    const g = conf.plugins && conf.plugins.widgets && conf.plugins.widgets.appGroup;
    if (!g) { process.stdout.write('group.app.demerzel.orchestrator'); process.exit(0); }
    process.stdout.write(String(g));
  " "$CONF")"
fi

{
  echo '<?xml version="1.0" encoding="UTF-8"?>'
  echo '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">'
  echo '<plist version="1.0">'
  echo '<dict>'
  echo '    <key>com.apple.security.application-groups</key>'
  echo '    <array>'
  echo "        <string>${GROUP}</string>"
  echo '    </array>'
  echo '</dict>'
  echo '</plist>'
} >"$EXT_ENTS"
echo "[siri] Entitlements App Group: $GROUP"

# Sync appGroup string in Swift sources if present.
for swift in "$SCRIPT_DIR"/Sources/*.swift; do
  [[ -f "$swift" ]] || continue
  if grep -q 'static let appGroup = "' "$swift" 2>/dev/null; then
    sed -i '' "s/static let appGroup = \"[^\"]*\"/static let appGroup = \"${GROUP}\"/" "$swift"
  fi
done

if command -v xcodegen &>/dev/null; then
  echo "[siri] Generating Xcode project..."
  cd "$SCRIPT_DIR"
  xcodegen generate --spec project.yml 2>&1 | tail -5
  cd - >/dev/null
else
  echo "[siri] xcodegen not found, expecting .xcodeproj already exists"
fi

XCODEPROJ="$SCRIPT_DIR/DemerzelAppIntents.xcodeproj"
if [[ ! -d "$XCODEPROJ" ]]; then
  echo "ERROR: $XCODEPROJ not found. Install xcodegen: brew install xcodegen" >&2
  exit 1
fi

echo "[siri] Building DemerzelAppIntents ($CONFIGURATION)..."
# Native arch only — universal (x86_64+arm64) often fails on Apple Silicon CI/dev.
xcodebuild build \
  -project "$XCODEPROJ" \
  -scheme "DemerzelAppIntents" \
  -configuration "$CONFIGURATION" \
  -derivedDataPath "$DERIVED_DATA" \
  ONLY_ACTIVE_ARCH=YES \
  ARCHS="$(uname -m)" \
  CODE_SIGN_IDENTITY="-" \
  CODE_SIGNING_REQUIRED=NO \
  CODE_SIGNING_ALLOWED=NO \
  2>&1 | tee /tmp/demerzel-siri-build.log | tail -30
if ! grep -q 'BUILD SUCCEEDED' /tmp/demerzel-siri-build.log; then
  echo "[siri] compile errors:" >&2
  rg -n 'error:' /tmp/demerzel-siri-build.log | head -40 >&2 || true
  exit 1
fi

APPEX_PATH="$DERIVED_DATA/Build/Products/$CONFIGURATION/DemerzelAppIntents.appex"
# ExtensionKit sometimes emits .appex under Products or under a nested path.
if [[ ! -d "$APPEX_PATH" ]]; then
  APPEX_PATH="$(find "$DERIVED_DATA/Build/Products/$CONFIGURATION" -maxdepth 3 -name 'DemerzelAppIntents.appex' -type d | head -1 || true)"
fi

if [[ -z "${APPEX_PATH:-}" || ! -d "$APPEX_PATH" ]]; then
  echo "ERROR: DemerzelAppIntents.appex not found under $DERIVED_DATA/Build/Products/$CONFIGURATION" >&2
  find "$DERIVED_DATA/Build/Products" -maxdepth 4 -type d 2>/dev/null | head -40 || true
  exit 1
fi

rm -rf "$APPEX_PATH/Contents/Frameworks"

if [[ -f "$EXT_ENTS" ]]; then
  codesign --force --options runtime --sign "$IDENTITY" \
    --entitlements "$EXT_ENTS" \
    "$APPEX_PATH"
  echo "[siri] Signed .appex ($IDENTITY)"
else
  codesign --force --options runtime --sign "$IDENTITY" "$APPEX_PATH"
fi

# Canonical path for tauri.conf.json bundle.macOS.files
CANON="$DERIVED_DATA/Build/Products/$CONFIGURATION/DemerzelAppIntents.appex"
if [[ "$APPEX_PATH" != "$CANON" ]]; then
  mkdir -p "$(dirname "$CANON")"
  rm -rf "$CANON"
  cp -R "$APPEX_PATH" "$CANON"
  APPEX_PATH="$CANON"
fi

echo "[siri] Built: $APPEX_PATH"
echo "[siri] Ready for bundle.macOS.files → Contents/PlugIns/"
