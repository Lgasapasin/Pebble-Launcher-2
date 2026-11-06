#!/usr/bin/env python3
"""Fail fast if a partial GitHub paste/upload has truncated the launcher source."""
from pathlib import Path
import sys

root = Path(__file__).resolve().parent.parent
required = {
    "src-tauri/src/minecraft.rs": (
        300,
        [
            "use crate::{auth, models::*, net, storage};",
            "pub async fn install_version(",
            "async fn extract_natives(",
            "pub async fn launch_game(",
            "fn expand_arguments(",
            "// End of complete minecraft.rs",
        ],
    ),
    "src-tauri/src/auth.rs": (150, ["pub async fn complete_login(", "async fn minecraft_tokens("]),
    "src-tauri/src/models.rs": (100, ["pub struct VersionMeta", "pub struct InstallProgress"]),
    "src-tauri/src/net.rs": (50, ["pub async fn ensure_download("]),
    "src-tauri/src/lib.rs": (20, ["minecraft::launch_game", "auth::complete_login"]),
}
errors = []
for filename, (min_lines, markers) in required.items():
    file = root / filename
    if not file.is_file():
        errors.append(f"MISSING: {filename}")
        continue
    contents = file.read_text(encoding="utf-8")
    lines = len(contents.splitlines())
    if lines < min_lines:
        errors.append(f"TRUNCATED: {filename} has only {lines} lines (expected >= {min_lines})")
    for marker in markers:
        if marker not in contents:
            errors.append(f"INCOMPLETE: {filename} is missing {marker!r}")
workflow = (root / ".github/workflows/build.yml")
if workflow.is_file():
    text = workflow.read_text(encoding="utf-8")
    if "build:appimage" not in text:
        errors.append("Workflow must build AppImage only using npm run build:appimage")
else:
    errors.append("MISSING: .github/workflows/build.yml")
if errors:
    print("\n".join(errors), file=sys.stderr)
    print("Upload the COMPLETE repaired source ZIP, not a pasted fragment.", file=sys.stderr)
    sys.exit(1)
print("Source completeness check passed (including full minecraft.rs).")
