# Codex Compatibility

Verified on Windows on 2026-09-06 for Codex Baoan 0.2.3.

| Client | Version | Evidence |
| --- | --- | --- |
| Codex CLI | 0.153.4 | Local `codex --version`, npm latest, upstream release |
| Codex Desktop (Windows) | 26.901.5280.0 | Installed `OpenAI.Codex` package manifest |
| Desktop bundled core | 0.153.4 | `app/resources/codex.exe --version` on a copy outside WindowsApps |

The desktop package version and its bundled core version are separate. This baseline does not assert that every Microsoft Store rollout has the same package version.

## Protocol Sources

The implementation was checked against the [0.153.4 release](https://github.com/openai/codex/releases/tag/rust-v0.153.4) and its source:

- [Rollout persistence policy](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/rollout/src/policy.rs)
- [TurnItem types](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/protocol/src/items.rs)
- [ResponseItem types](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/protocol/src/models.rs)
- [Configuration schema](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/core/config.schema.json)
- [Provider definitions](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/model-provider-info/src/lib.rs)

## Supported Records

| Record | Handling |
| --- | --- |
| `session_meta` | Client source, core version, legacy/paginated history mode |
| Legacy `response_item.function_call` | `shell`, `shell_command`, `exec_command`; `command` string/array and `cmd`; JSON strings or objects |
| Legacy parallel calls | Each `multi_tool_use.parallel.tool_uses` entry gets a separate event |
| Legacy `local_shell_call` | `action.type = exec` and command array |
| `event_msg.item_completed` / `CommandExecution` | Actual command, interactive input, failed/declined status |
| `event_msg.item_completed` / `FileChange` | Adds, deletes, updates, renames, line counts |
| Legacy `exec_command_end` / `patch_apply_end` | Completed commands and structured file changes |
| Code Mode output metadata | Recorded `executed_tool_calls` when present in legacy logs |

Paginated sessions use completed items instead of raw tool requests, preventing duplicate counting. JavaScript in a Code Mode `exec` input is never executed or interpreted as a shell command. Command output and patch contents are not copied into activity records. Failed patches are not counted as completed file changes.

## Configuration

- Discovery and monitoring share `CODEX_HOME`, falling back to the user's `.codex` directory.
- A named `profile` overrides the root model/provider selection. An unspecified provider means OpenAI, not the first entry in `model_providers`.
- File-backed API keys and ChatGPT login records, provider `env_key`, inline bearer tokens, and authentication headers are recognized locally.
- `keyring`, `auto`, ephemeral login, and command/AWS authentication remain unverified. No credential-store access, authentication commands, or upstream requests are needed for monitoring.
- The displayed provider comes from the configured Codex home. Per-process CLI flags, project overrides, remote environments, and a different home used by another process are not resolved from that file.

## Verification

```powershell
pnpm run typecheck
pnpm run build:renderer
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
```

Two optional, read-only smoke tests can inspect the local installation. They print only source/version, counts, and authentication presence, never credentials or commands:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib local_ -- --ignored --nocapture
```

To replay a particular desktop or CLI rollout, set `CODEX_BAOAN_SMOKE_ROLLOUT` to its absolute path before running `local_rollout_smoke`. With a populated Cargo cache, `--offline` avoids registry availability affecting local verification.

On Windows GNU, unit tests explicitly link Tauri's generated resources so the test executable also receives the Common Controls v6 manifest. The regular application keeps Tauri's normal resource handling. For Scoop installations, ensure the Rust toolchain's `bin` directory is on the current process's `PATH` when invoking `cargo fmt`.

## Monitoring Bounds

- This is local log auditing, not execution prevention. New paginated commands appear after Codex persists their completed items.
- Startup imports the last 1 MiB of the 8 most recently modified sessions. All other existing sessions are tracked from their current end; newly appended records are collected when an old session resumes.
- The monitor polls approximately every 800 ms and discovers new files approximately every 4 seconds. Codex's own log buffering can add delay.
- Reading is limited to 1 MiB per session per tick. Incomplete UTF-8/JSON lines are retained; malformed complete records and records larger than 16 MiB are skipped. Symlinked directories are not traversed.
- Only local `sessions/rollout-*.jsonl` files are monitored. Archived/compressed history and remote-only sessions are not imported.
- Event deduplication is scoped to each rollout. The application retains 500 events in memory and returns the latest 120 to the interface.

When upgrading Codex again, check the rollout persistence policy and item schema first, then replay logs from both clients. A CLI version check alone does not verify a desktop build.
