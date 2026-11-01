#!/usr/bin/env bash
# Локальная проверка на Mac с отчётом (#133 и далее).
# Использование:
#   bash scripts/mac-check.sh            # тесты фронта и бэкенда, отчёт в reports/
#   bash scripts/mac-check.sh --build    # + tauri build --debug (долго)
#   bash scripts/mac-check.sh --push     # + коммит и push отчёта в текущую ветку
#   Авто: `bun run test` на Mac запускает проверки и пушит отчёт в текущую ветку (не в main).
# Результат: reports/mac-check-<дата>.md. Я читаю его в следующей итерации из репозитория.
set -u
cd "$(dirname "$0")/.." || exit 1

DO_BUILD=0; DO_PUSH=0
for a in "$@"; do
  case "$a" in
    --build) DO_BUILD=1 ;;
    --push) DO_PUSH=1 ;;
  esac
done

STAMP=$(date +%Y%m%d-%H%M)
REPORT="reports/mac-check-$STAMP.md"
mkdir -p reports

section() { printf '\n## %s\n\n' "$1" >> "$REPORT"; }
run() {
  # run "<название>" <команда...>: пишет код выхода и последние строки вывода
  local name="$1"; shift
  local log; log=$(mktemp)
  "$@" >"$log" 2>&1
  local code=$?
  if [ $code -eq 0 ]; then status="OK"; else status="FAIL ($code)"; fi
  printf -- '- **%s**: %s\n' "$name" "$status" >> "$REPORT"
  if [ $code -ne 0 ]; then
    printf '\n```\n' >> "$REPORT"; tail -n 40 "$log" >> "$REPORT"; printf '```\n' >> "$REPORT"
  fi
  rm -f "$log"
  return $code
}

{
  echo "# Mac check $STAMP"
  echo
  echo "- Ветка: $(git rev-parse --abbrev-ref HEAD) @ $(git rev-parse --short HEAD)"
  echo "- macOS: $(sw_vers -productVersion 2>/dev/null || echo n/a), $(uname -m)"
  echo "- bun: $(bun --version 2>/dev/null || echo 'не найден')"
  echo "- cargo: $(cargo --version 2>/dev/null || echo 'не найден')"
} > "$REPORT"

section "Фронтенд"
run "bun install --frozen-lockfile" bun install --frozen-lockfile
run "bun test (src/lib)" bun test src/lib
run "bun run check" bun run check
run "bun run build" bun run build

section "MCP (#158)"
run "bun test mcp" bun test mcp
run "MCP initialize (stdio)" sh -c 'printf "%s\n" "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}" | node mcp/demerzel-mcp.mjs | grep -q "\"serverInfo\""'

section "Бэкенд (Rust)"
run "cargo test --lib" cargo test --manifest-path src-tauri/Cargo.toml --lib
run "cargo test: backend::env (#133 PATH)" cargo test --manifest-path src-tauri/Cargo.toml --lib backend::env
run "cargo clippy -D warnings" cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings

section "Проверка PATH для процессов (#133)"
# Тот же вызов, что делает приложение: sh -c без PATH оболочки
if command -v bun >/dev/null; then
  BUN_DIR=$(dirname "$(command -v bun)")
  printf -- '- bun в: `%s`\n' "$BUN_DIR" >> "$REPORT"
  if env -i /bin/sh -c 'command -v bun' >/dev/null 2>&1; then
    printf -- '- без PATH оболочки bun виден: да\n' >> "$REPORT"
  else
    printf -- '- без PATH оболочки bun виден: нет (проверить фикс #133 в приложении)\n' >> "$REPORT"
  fi
fi

if [ $DO_BUILD -eq 1 ]; then
  section "Сборка Tauri (debug)"
  run "bun run tauri build --debug" bun run tauri build --debug
fi

section "Итог"
if grep -q "FAIL" "$REPORT"; then
  echo "Есть падения, см. выше." >> "$REPORT"
else
  echo "Всё зелёное." >> "$REPORT"
fi
echo "Отчёт: $REPORT"
cat "$REPORT"

if [ $DO_PUSH -eq 1 ]; then
  BRANCH=$(git rev-parse --abbrev-ref HEAD)
  if [ "$BRANCH" = "main" ]; then
    echo "Отчёт не запушен: сейчас main. Переключитесь на рабочую ветку."; exit 0
  fi
  git add "$REPORT"
  git commit -m "chore(reports): mac check $STAMP" && git push && echo "Отчёт запушен"
fi
