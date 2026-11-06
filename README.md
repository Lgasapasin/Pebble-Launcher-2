# Pebble Launcher v0.1 — Minecraft Java Edition (AppImage CI build)

A minimal Tauri 2 + Rust + TypeScript launcher. **Early source prototype — not an official Mojang/Microsoft product.**

**To build without compiling on your PC:** see [`FIXES-AND-UPLOAD.md`](FIXES-AND-UPLOAD.md) for instructions to update GitHub and run the **AppImage-only** workflow. The GitHub repository's previous `minecraft.rs` was truncated; use this complete archive rather than pasting partial code. This archive contains source files, **not a tested executable**. Authentication and launching require end-to-end testing on a machine with Rust, system dependencies, Microsoft app permissions, and an installed Minecraft Java Edition account.

## Included in this prototype

- Compact dark UI, with Instances / Accounts / Settings / Console tabs.
- Fetch version metadata from Mojang's official version manifest; list releases and snapshots.
- Install selected Vanilla versions: client JAR, classpath libraries, OS-specific native classifiers, asset index + assets, logging config where present.
- SHA-1 verification of downloads where hashes are supplied; bounded concurrent downloads; basic repair by reinstalling.
- Isolated game folders for each installed Minecraft version and a common asset/library cache.
- Microsoft **device-code sign-in** (no password entered into the launcher), Xbox Live + XSTS exchange, Minecraft profile lookup, token refresh, and OS keyring persistence when available.
- Launch Java with metadata-driven arguments, native library path, memory limit, and log event streaming.
- Java major-version check before launch.

## Explicit limitations / unverified work

- This is an **untested early prototype**, not a validated working Minecraft release. The Rust backend, native dependencies, Java launch, and Microsoft/Xbox authentication have **not** been compiled or exercised end-to-end in the authoring environment.
- Authentication requires an **Azure public client app ID** configured for personal Microsoft accounts, support for device-code public-client flow, and authorization to access the Minecraft services API where Microsoft/Mojang require it. Without approval, sign-in may fail with an API 403. We have **not** created an app registration for you.
- A Microsoft account that owns Minecraft Java Edition is required to play. No offline-authentication bypass or cracked account support.
- The launcher **does not install Java automatically**; select a matching Java executable in Settings. Modern versions may require Java 21/25. Java version is obtained from the Minecraft version metadata, not hardcoded by Minecraft version number.
- Very old releases using `minecraftArguments` instead of the modern `arguments` metadata format are not supported. Some older assets formats, native library layouts, and all complex mod-loader chains are not yet supported.
- Snapshot handling and native/classpath OS rules need real-game testing; no automated OS-level Minecraft smoke test has been performed.
- Only a basic **version-per-game-folder** arrangement is included; named custom instances, multiple profiles per version, managed Java, Fabric/Forge/NeoForge/Quilt, mod installers, and modpacks are planned for future versions.
- Current login uses a manually entered Microsoft application client ID, not a shared publisher-issued client ID.
- Game token is passed to Java as a normal Minecraft launch argument (as is standard for launchers); it may be visible to local operating-system process inspection. Logs attempt to redact token strings.
- On systems without a working OS keyring, login is held only in memory for that session. Tokens are never intentionally written to unencrypted app settings.
- No cancellation button, download resume, multi-instance locks, or auto-updater yet.

## Requirements

- Linux x86-64 (Mint 22 / Ubuntu 24.04 recommended) or Windows x86-64.
- Node.js 22+, npm, Rust + Cargo (`rustup`), and Tauri v2 development prerequisites.
- Network access to Mojang/Microsoft download and identity endpoints.
- A compatible Java runtime for each Minecraft version.

### Linux Mint 22 / Ubuntu

Install native Tauri prerequisites (from [Tauri's official prerequisites](https://v2.tauri.app/start/prerequisites/)):

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

Install Rust using the official instructions: <https://rustup.rs/>. Then:

```bash
cd pebble-launcher-v0.1
npm install
npm run check
npm run dev
```

To build a distributable Linux package:

```bash
npm run build
```

The output appears under `src-tauri/target/release/bundle/` if the build succeeds. Linux Tauri builds commonly produce Debian packages and AppImages (depending on available tools).

### Windows

Install Node.js 22+, Rust + Cargo with the MSVC toolchain, Microsoft C++ Build Tools, and the Windows WebView2 prerequisites as described in [Tauri's Windows prerequisites](https://v2.tauri.app/start/prerequisites/). From PowerShell in the project directory:

```powershell
npm install
npm run check
npm run dev
npm run build
```

The Windows installer/build files appear under `src-tauri/target/release/bundle/` after a successful native build. Use a Windows machine or Windows CI runner for Windows builds; the provided archive **does not include** a precompiled Windows installer.

## Sign-in setup (required before Play)

1. In Microsoft's Azure/Entra application registration portal, register a public/native application that allows **personal Microsoft accounts**, enable public-client / device-code flow, and obtain its **Application (client) ID**. Never embed a client secret.
2. Confirm that the app has the required authorization for Xbox Live/Minecraft services APIs. New apps may need Minecraft API permission approval; the launcher cannot bypass that restriction.
3. Open **Accounts** in Pebble Launcher, paste your client ID, and select **Sign in with Microsoft**.
4. Open the displayed verification URL **manually in your normal browser**, enter the displayed code, and approve only the permissions you recognize. The launcher should show your Minecraft Java profile after the official flow succeeds.
5. In **Settings**, provide the correct Java runtime path. In **Instances**, select a release, Install, then Play.

Sign-in uses `XboxLive.signin offline_access`. Refresh tokens are stored through the operating system credential store when supported. No passwords or token files should be placed in this project directory.

## Project structure

```text
src/                  # Framework-free TypeScript, CSS UI
src-tauri/src/
  auth.rs             # Device code, MS/Xbox/XSTS/Minecraft auth, keyring
  minecraft.rs        # Version download, verification, natives, launching
  models.rs           # Mojang manifest types
  net.rs              # HTTP/download and SHA-1 integrity helpers
  storage.rs          # App data and path safety
  lib.rs              # Tauri commands, managed state
src-tauri/icons/      # Bundled launcher icons
src-tauri/capabilities/default.json
```

## Next engineering steps

1. Install Rust dependencies and compile `cargo check` on Linux and Windows; fix any API/toolchain compatibility issues.
2. Exercise sign-in with an approved Microsoft app registration and a licensed test account.
3. Install/launch a modern release on both target systems and compare the JVM launch command and logs against official metadata.
4. Add manifest-fixture tests (OS rules, natives, asset integrity, and placeholder expansion), retry + cancellation, per-host rate limits, and better error reporting.
5. After V0.1 is tested, implement named instances, managed Java, and mod loaders.

## Terms

Minecraft belongs to Mojang/Microsoft. Pebble Launcher is an independent experimental project, not affiliated with or endorsed by Mojang or Microsoft. Minecraft game files are obtained from official service endpoints; users are responsible for complying with applicable terms and for using licensed accounts.
