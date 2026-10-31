# Releases and in-app updates

Demerzel checks GitHub Releases for updates shortly after launch and every six hours. When a new signed release is available, the app shows its version and release notes; the user chooses when to install and restart.

## One-time setup

The Tauri updater verifies update archives with a Minisign key. Keep `src-tauri/updater.key` private; it is ignored by Git and created with mode `0600`. The matching public key is committed in `src-tauri/updater.key.pub` and embedded in `src-tauri/tauri.conf.json`.

In GitHub, open **Settings → Secrets and variables → Actions** and add repository secret `TAURI_SIGNING_PRIVATE_KEY` with the full contents of `src-tauri/updater.key`. The key currently has no password, so `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` is not needed. Back up the private key securely: losing it prevents signing future updates for installations that trust this public key.

## Publish a release

1. Set the same SemVer in `VERSION`, `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.
2. Commit and push the version change to `main`.
3. Create and push the matching tag, for example `git tag v0.55.22` then `git push origin v0.55.22`.
4. The `Release` workflow verifies the tag, builds a universal macOS app for Apple Silicon and Intel, creates the `.dmg` and signed updater archive, and publishes them with `latest.json` to a GitHub Release.

The workflow requires the signing secret and `contents: write`. Pull request CI uses a disposable signing key and never needs the release key.

## First updater-enabled version

Install the first release that contains the updater plugin manually. Versions installed before that release do not contain updater support and cannot update themselves. Later versions can be installed from the in-app **Обновления** button.
