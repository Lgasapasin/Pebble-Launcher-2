# Build Pebble Launcher on GitHub (no Rust or Cargo on your PC)

This repository contains a GitHub Actions workflow to compile Linux and Windows installers on GitHub-hosted runners. No local compilation is necessary.

## Start a build

1. Sign in to https://github.com and create a new repository (e.g. `pebble-launcher`).
2. Extract the source ZIP. Upload the **contents** of the `pebble-launcher-v0.1` directory to the repository root, including the hidden `.github/workflows/build.yml` file. **Do not upload the ZIP alone.**
3. Verify that the GitHub repository shows `package.json`, `src-tauri/`, `src/`, and `.github/workflows/build.yml` at the correct paths.
4. Open the **Actions** tab. If GitHub asks whether to enable workflows, review the workflow and enable it.
5. Select **Build Pebble Launcher (Linux + Windows)**, then **Run workflow**. The workflow also runs automatically on new commits pushed to `main`.
6. Open the completed workflow run, scroll to **Artifacts**, and download the Linux or Windows installer artifact. You may have to unzip the artifact to obtain the installer.

Expected outputs:

- Linux: `.deb` and `.AppImage`
- Windows: NSIS `.exe` installer

## Notes

- The current source is an early prototype. This workflow has **not yet been run** on GitHub; Rust compilation and runtime behavior must be verified from the first build logs. If the build fails, copy the failing job log for troubleshooting.
- A successfully compiled app is not proof that Minecraft login or launching works. Microsoft authorization requires an appropriate Microsoft app registration with Minecraft API access, and Minecraft must be tested with an authorized account.
- Windows installers from this workflow are not code-signed, so Windows SmartScreen may warn about unknown publishers. Do not disable Windows security protections to run an untrusted installer.
- GitHub Actions usage is free for standard GitHub-hosted runners in public repositories. Private repository usage is subject to included minutes/storage and possible billing.
- The executable and installer are GitHub-hosted workflow artifacts; there is no automatic public release or code signing configured.
- To prevent accidental workflow execution on every push, remove the `push` trigger in `.github/workflows/build.yml` and use manual **Run workflow** only.
