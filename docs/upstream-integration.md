# ELYTerminal Upstream Integration

## Baseline

- MioTerm base: `72862bb` (`v0.0.8`).
- Previously included upstream: `f360143` (ELYTerminal 0.1.7).
- Integrated upstream: `6936b6f3edb5f55891dd62898290994f52e24dc0` (0.2.0).
- Upstream repository: https://github.com/ELY-Open-Source/ELYTerminal.
- MioTerm keeps its own version, release tags and update endpoint.
- MioTerm `v0.0.9` is the first release containing this integration.

## Included

- English and Simplified Chinese upstream UI and live language switching.
- Custom themes, background images, saved display and window position.
- Session reordering, duplicate/close menus and optional local Files menus.
- Configurable start folder and optional macOS natural text editing.
- Kitty graphics, animations, OSC 5522 clipboard with read approval.
- Packed idle scrollback, ASCII parsing and text shaping improvements.
- Windows fresh shell environment, Git Bash, WSL paths and WSL Git.
- Bundled Microsoft ConPTY, Windows input/output fixes and Mica options.
- Linux XIM decoding and Wayland modifier fixes.
- Linux tar, DEB, RPM and AppImage build scripts and third-party notices.

The terminal engine and platform patches remain pinned to upstream revisions
in `Cargo.toml`. They are not floating dependencies.

## MioTerm Behavior Preserved

- SSH host management, PEM imports, managed keys and askpass dispatch.
- Local/remote file panes, SFTP uploads, downloads and file drops.
- Remote Git and monitoring, following the managed SSH session's directory.
- Gitee/WebDAV encrypted synchronization and the encrypted application vault.
- Existing ELYTerminal-named data directories; no user-data migration on start.
- Closed sidebars on every startup, icon switches and Mocha's pink accent.
- MioTerm release URLs and a single workflow triggered only by version tags.

SSH duplication reconnects with the original profile and a separately encoded
remote start directory. Local new-session actions never run the SSH executable
as though it were the configured local shell. WSL Git routes through the local
backend only; remote Git always runs on its selected SSH host.

The upstream website and its additional push/PR packaging workflow are excluded.
The imported `docs/en_us` and `docs/zh_cn` are upstream terminal references,
not a replacement for MioTerm's README or SSH documentation.

## Post-Merge Review

- Managed SSH sessions accept only directly streamed Kitty graphics data.
  File, temporary-file and shared-memory media are refused before loading,
  including queries and animation frames; local sessions retain those media.
- Managed SSH OSC 7 reports always use remote POSIX parsing, even for an
  empty authority, `localhost` or a hostname matching the client. Windows
  drive and UNC conversion remains limited to ordinary local sessions.
- The session flag is fixed at launch. Manually running `ssh` inside a local
  terminal does not turn that terminal into a managed SSH session.

Regression tests reproduced both bugs before their fixes. They cover client
file preservation, shared-memory refusal, partial-transmission cancellation,
streamed pixels and placements, and OSC 7 routing through an actual PTY.

## Validation

Use the pinned Rust 1.95.0 toolchain. On local macOS installations without a
precompiled Metal library, add `--features gpui_platform/runtime_shaders`.

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Tests use generated keys, isolated app folders and local mock sync servers.
Live Gitee tests remain opt-in and must not upload real user configuration.
Platform-specific runtime behavior requires the corresponding operating system.

### Local Integration Checks

On macOS with Rust 1.95.0:

- Release binary and native screenshot harness build successfully.
- Clippy passes for all targets with warnings denied; formatting passes.
- Core regression after review and sync fixes: 518 passed, one opt-in Gitee test ignored.
- Four shell-dependent tests require unavailable fish or PowerShell locally.
- Application vault and actual askpass helper tests pass in an isolated folder.
- Generated encrypted RSA PEM: import, delete source, SSH login and SFTP upload pass against loopback sshd.
- Both opt-in loopback SSH tests pass: remote Git actions, SFTP upload/download and shell directory reporting.
- Native screenshots confirm Kitty PNG rendering, closed startup sidebars, Mocha pink accent and the desktop character.
- Native clicks switch Chinese to English immediately and retain the Sync settings page.
- Actionlint, shell syntax, whitespace and the 500-line rule pass.

No real user hosts, keys or cloud archives were read or synchronized.
Windows and Linux native runtime/package installation checks are not run on this
macOS host; Docker and those desktop environments are unavailable.

Some screenshot runs with the desktop character enabled log GPUI's
`RefCell already borrowed` during popup creation. The terminal and character
render and exit normally; runs with the character disabled do not reproduce it.
This nonfatal window-callback diagnostic remains a platform verification note.
