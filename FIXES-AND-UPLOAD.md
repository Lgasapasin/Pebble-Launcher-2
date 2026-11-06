# Pebble Launcher — full project repair (AppImage only)

## What failed
The GitHub copy of `src-tauri/src/minecraft.rs` was **truncated** to ~97 lines. It began in the middle of `launch_game()` and therefore had an `unexpected closing delimiter`. The project ZIP uploaded for repair contained the full 371-line module.

## What changed
- Retained **all original project files** from the uploaded ZIP.
- Repaired download stream ownership: `version_id` is cloned into a move-closure; `client` and `app` are cloned before the closure, avoiding E0505 at the final progress event.
- Added `scripts/check_source.py` to reject truncated Rust source files **before** dependency compilation.
- Added a Rust syntax/format pass via `cargo fmt --all` to the workflow.
- Added `build:appimage` to `package.json`; CI builds **only** Linux x86-64 AppImage.
- Added `.gitignore` for build outputs and secrets.

## Upload the ZIP correctly
Do not edit `minecraft.rs` by pasting snippets. Unzip the archive and **replace the repository's contents with the extracted project files**, preserving directories (including the hidden `.github/workflows/build.yml`).

### Reliable method: Git command-line (does not compile anything)
On Linux Mint, with `git` installed:

1. Clone your existing repository:
   `git clone https://github.com/Lgasapasin/Pebble-Launcher.git`
2. Extract this fixed archive elsewhere, then copy **the files inside** `Pebble-Launcher-main/` into your clone, including `.github` (overwrite matching files).
3. Run from the cloned repository:

   ```bash
   python3 scripts/check_source.py
   git add -A
   git commit -m "Fix complete Minecraft module and AppImage workflow"
   git push origin main
   ```

Or use GitHub web uploader, but check `src-tauri/src/minecraft.rs` after upload: it must be **over 300 lines** and **begin with** `use crate::{auth, models::*, net, storage};` near the top, **not** `tokio::fs::create_dir_all(&game_dir)`.

## Build
GitHub → repository → Actions → **Build Pebble Launcher (AppImage only)** → **Run workflow**.
The workflow first verifies source completeness, formats/parses the Rust files, typechecks the UI, then builds a Linux AppImage and uploads it as an artifact.

## What is verified vs not verified
Validated offline: ZIP/source layout, source completeness, TS typechecking with bundled shims, and workflow structure. This execution environment has **no Rust/Cargo** and no outgoing network access, so a real Rust compile, bundling, and Minecraft login/game launch **must be validated by GitHub Actions and runtime tests**. Do not treat this as a compiled release yet.
