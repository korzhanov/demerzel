# Отчёты проверок на Mac

Запуск на Mac из корня репозитория:

    bash scripts/mac-check.sh --push

Отчёт `reports/mac-check-<дата>.md` коммитится и пушится; в следующей итерации я читаю его из репозитория. Флаг `--build` добавляет сборку `tauri build --debug`.
