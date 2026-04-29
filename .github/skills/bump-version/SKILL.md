---
name: bump-version
description: Bump the Cable app version. Use when the user asks to bump/update/release a new version, or change the version number.
allowed-tools: edit, view
---

# Cable Version Bump

Use this skill to update the Cable app version across all relevant files.

## Files to update

These are the exact three files that contain the version, and the pattern to replace:

| File | Pattern |
|------|---------|
| `package.json` | `"version": "OLD"` → `"version": "NEW"` |
| `crates/tauri/Cargo.toml` | `version = "OLD"` (line 3, the package version) → `version = "NEW"` |
| `crates/tauri/tauri.conf.json` | `"version": "OLD"` → `"version": "NEW"` |

## Workflow

1. Use `ask_user` to ask for the new version number (freeform, e.g. `1.2.3`).
2. Read the current version from any one of the three files above to confirm the old value.
3. Apply the edit to all three files using the `edit` tool.
4. Confirm to the user which files were updated and what the new version is.

## Notes

- `tools/vst-inspect/Cargo.toml` is an independent utility — do **not** update it.
- Do not run builds or tests as part of this skill; version bumping is a file-only change.
- `Cargo.lock` will reflect the new version automatically on the next build.
