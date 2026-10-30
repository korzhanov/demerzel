# Changelog

## 0.1.2 — 2026-10-30

- Инструменты: Bun вместо pnpm (`bun.lock`, `beforeDevCommand`/`beforeBuildCommand`, CI через `oven-sh/setup-bun`).
- Конвенции: Bun и шаг обновления ссылки на удалённую ветку после push.

## 0.1.1 — 2026-10-30

- CI: GitHub Actions. Frontend на Ubuntu (`pnpm check`, `pnpm build`), backend на macOS (`cargo test`, `cargo clippy -D warnings`, `pnpm tauri build --debug`). Закрывает #34.

## 0.1.0 — 2026-10-30

- Скелет приложения: Tauri 2 + SvelteKit в режиме SPA (PR, закрывает #3).
- Переименование пакета и продукта в Demerzel, `ssr = false`, `adapter-static`.
- `.gitignore` для Node, Rust и сборки.

## 0.0.1 — 2026-10-30

- Переименование проекта в Demerzel (PR #docs-conventions).
- Метки для issues: фазы и области.
- Конвенции: ветки, коммиты, версии, журналы.
- Ассеты перенесены в `references/media/`, добавлены `references/README.md` и каркас `wireframes/`, `mockups/`.
