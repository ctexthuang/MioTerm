# ELYTerminal Upstream Integration

## Baseline

- MioTerm base: `ef22283` (`v0.1.0`).
- Previously included upstream: `fdca49aca4400467ed08bd8d873f82348de797a3` (0.2.3).
- Integrated upstream: `9504b5382f3b47b0377063fa3b78cb706f44ed08` (0.2.4, fetched 2026-11-11).
- Upstream repository: https://github.com/ELY-Open-Source/ELYTerminal.
- MioTerm keeps its own version, release tags and update endpoint.
- MioTerm `v0.1.1` integrates upstream 0.2.3 and the follow-up review fixes.
- Current source integrates upstream 0.2.4; no new MioTerm release tag is assigned.

## New Since Upstream 0.2.3

- A configurable keymap with recording, reset and conflicting-key validation.
- Clear screen/history, retaining the prompt on Unix and history-only on Windows.
- Optional authenticated local CLI: list sessions, read output, send text or keys.
- Independent picture opacity, gradient colors/angle and a CSS-like gradient mask.
- A folder chooser in the profiles menu and terminal focus after a file-tree drop.
- Windows NSIS installers, Insert copy/paste keys and PowerShell policy fixes.
- Narrow sidebar toggles and pixel-aligned Wayland desktop-character positioning.
- Terminal name/version environment variables, propagated through WSL.

GPUI, Ely and the platform patches stay pinned to the same revisions. Rust stays
at 1.95.0. Windows installers use MioTerm names, registry keys and URLs and join
the existing version-tag-only release workflow; upstream CI and website workflows
stay excluded. The existing ELYTerminal data-directory names stay intact.

### SSH Adaptation

- Both sidebars remain closed at startup; SSH Hosts and Sync settings remain.
- All custom terminal shortcuts, history clearing and backgrounds apply to SSH.
- CLI lists remote POSIX folders, SSH endpoints and connection status; disconnected
  sessions allow reading retained output but reject text and key input.
- CLI uses the same `MIOTERM_CONFIG_DIR` override as settings, including isolated
  test runs. Its token file has user-only Unix permissions or Windows ACLs and
  stays outside encrypted SSH cloud archives.
- Local and managed remote shells report `TERM_PROGRAM=mioterm` with this fork's
  version, without depending on a server's `AcceptEnv` setting.
- The profiles menu's folder chooser uses the active session's local or remote
  backend. Local profiles remain local even while a remote session is active.
- Remote tree-to-terminal drags resolve only the active SSH pane's current keys,
  quote paths for the remote shell and focus its terminal. Local-to-remote and
  remote-to-local SFTP drags retain their transfer confirmation and task queue.
- Existing picture size/position overflow checks are preserved with the new mask.

The Windows typed-space issue remains an upstream diagnostic change, not a fix.

### Current 0.2.4 Checks

On macOS with runtime shaders, 698 core tests pass, one opt-in cloud test stays
ignored and four shell/environment-dependent tests are filtered. Font loading,
encrypted application-vault and real askpass tests also pass. Three explicit
loopback SSH tests pass: remote Git/SFTP, changed transfer-path types, and remote
shell directory/terminal-version reports. All fixtures use generated keys and
isolated configuration; no production SSH hosts or cloud stores are used.

Native macOS windows additionally verify custom Clear/New session shortcuts,
local and SSH CLI input/read/key operations, remote directory listing and refusal
of input after SSH disconnects. SSH Clear and a remote filename containing a
space and quote pass a real tree-to-terminal drop with subsequent typing. A
local-to-remote UI drag, confirmation and SFTP upload produce matching contents,
and the inline task area opens automatically and displays the completed job.

All-target Clippy with denied warnings, formatting, whitespace and the 500-line
rule pass. The release binary and screenshot harness build. The release YAML
passes Actionlint 1.7.7 with its outdated `windows-11-arm` label warning excluded;
the hosted ARM runner and the single tag-only trigger remain unchanged.

Manual verification is described in [0.2.4 testing](upstream-024-testing.md).
Windows/Linux native runtime and Windows installer execution require those
platforms and are not verified by a macOS test run.

## New Since Upstream 0.2.2

- Background opacity now accepts 0 to 100 percent.
- Background images can fill the terminal or the window below its title bar.
- Image size, position and repetition accept CSS-like settings.
- Links gain hover underlines, with bounded scanning for plain URLs.
- Optional right-click copy/paste, enabled by default only on Windows.
- Windows mouse wheel reports count notches rather than configured scroll lines.
- Windows image-only clipboard pastes notify bracketed-paste applications.
- Windows keeps the pointer hidden while typing until it actually moves.
- Underline and box-drawing rule thickness rounds up to device pixels.
- Windows ARM64 testing and packaging join MioTerm's existing tag-only workflow.

GPUI remains pinned to 0.3.8, Ely to `294a832`. The Windows platform patch
advances to `0b74ab7`; the toolchain stays unchanged, and MioTerm releases as 0.1.1.
The shared session-row helper retains MioTerm's entity-based drag identities.

## New Since Upstream 0.2.0

- Named launch profiles with command arguments and optional start folders.
- Detected local shells, Git Bash and WSL distributions in the profile menu.
- Interface font and size, terminal font weight and a separate CJK font.
- Grayscale/subpixel antialiasing and configurable smooth cursor motion.
- Local Files polling, Git mark refresh and tree-to-terminal path dragging.
- Right-click the Files icon to choose a folder for a new local session.
- Folder CLI arguments and Linux `X-TerminalArgDir` integration.
- Per-user Windows Explorer menus branded MioTerm with separate registry keys.
- cmd prompt/directory integration and clearing previous command failures.
- zsh prompt resize/reflow fixes and CSI 16 t cell-pixel-size replies.
- Touch Bar Mac window-close crash fix in the pinned macOS platform dependency.

Upstream's Homebrew/winget manifests target ELYTerminal and are excluded, along
with its website and deployment workflow. MioTerm keeps its own release workflow.

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

Local Files polling pauses while the remote file panes are shown. MioTerm's
dual-pane Files now polls both panes and their Git marks once per second while
visible. The Git panel also polls local and remote repositories, serializing
refreshes with pending Git actions and preserving drafts. The Files tab's folder
action uses a native chooser for local sessions and an SSH directory browser
for managed remote sessions, reconnecting the same host in the chosen directory.
Hidden panels stop polling; slow requests coalesce and stale session results
are discarded. cmd OSC 9;9 reports remain local; managed SSH follows remote OSC 7.
The previously rolled-back synchronization redesign is not included.

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

The 0.2.3 follow-up review found that finite picture settings such as `3e38%`
can overflow during size or position calculations. Picture bounds are now
checked before tiling or rendering; invalid bounds log the responsible setting
and retain the window's ground. A regression test reproduced infinite bounds
before the fix, and a normal positioning test preserves ordinary layouts.

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

### Current 0.2.3 Integration Checks

On macOS with Rust 1.95.0 and runtime shaders:

- Core regression after review: 619 passed, one opt-in Gitee test ignored, four shell tests filtered.
- Isolated fonts, application vault, actual askpass and generated RSA PEM tests pass.
- All-target Clippy with warnings denied, formatting and the 500-line rule pass.
- Release binary and native screenshot harness build successfully.
- Native screenshots confirm closed startup sidebars and the desktop character.
- Terminal-only positioning and full-window horizontal image repetition render correctly.
- Host, Git and monitoring panels retain Mocha's pink accents with a background image.
- Native right-click events copy five selected fixture characters and paste five characters.
- Appearance exposes all picture controls; Settings retains the Sync page.
- Release YAML, version-tag-only trigger and ARM64 publication dependencies are checked.
- Both opt-in loopback SSH tests pass: remote Git actions, SFTP transfers and directory reports.
- Generated encrypted RSA PEM authentication and SFTP pass after the source key is removed.
- A managed SSH session shows the background image and the URL hover underline.
- Native SSH right-click paste delivers exactly `ESC[200~helloESC[201~` to a remote raw reader.

Link detection and hover cell tests pass, with the underline also confirmed in
a native SSH screenshot. The two loopback tests are ignored by default and were
explicitly run against an isolated local sshd with generated keys. No live Gitee
tests ran. Four shell tests require fish, PowerShell or platform-specific
environments absent on this host. Windows and Linux native runtime/package
checks remain outstanding. All test credentials and configuration are generated
fixtures; the local SSH master and server are closed after verification.

### Manual Checks for This Update

1. Start MioTerm and confirm both sidebars stay closed until clicked.
2. In Settings > Appearance > Window, select an image and switch Picture scope.
3. Try `contain`, `300px auto`, `right bottom` and horizontal repetition; toggle sidebars.
4. Move Background opacity to 0%, then restore the preferred opacity.
5. Print `https://example.com/path` in the terminal and hover the URL.
6. Enable Settings > Terminal > Right click pastes; select text and right-click twice.
7. Open Settings > Sync and confirm the existing Gitee/WebDAV options remain available.
8. Connect a test SSH host and check Files, Git and monitoring follow its directory.

### Previous 0.2.2 Integration Checks

On macOS with Rust 1.95.0 and runtime shaders:

- Core regression: 575 passed, one opt-in Gitee test ignored, four shell tests filtered.
- Isolated font, encrypted application vault and actual askpass helper tests pass.
- Generated RSA PEM import, source removal and passphrase conflict tests pass.
- All-target Clippy with warnings denied, formatting and the 500-line rule pass.
- Release binary and native screenshot harness build successfully.
- Native screenshots confirm closed startup sidebars, CJK text and Mocha pink.
- Native Files checks confirm automatic refresh and its right-click folder action.
- The profile menu detects shells and opens a saved command with its arguments and folder.
- The settings window shows interface font controls and retains Sync navigation.
- No live Gitee or loopback SSH tests ran for this integration.

The subsequent panel adaptation passes 581 core tests, all-target Clippy and the
release build. Native screenshots against an isolated loopback sshd verify dual
Files refresh with Git marks, local/remote Git branch and history polling, draft
preservation, and new SSH sessions in directories containing spaces and quotes.
Tests use generated keys and isolated known-host/configuration files; no real
server or cloud data is involved.

The four filtered tests require fish, PowerShell or platform-specific shell
environments absent on this host. Windows and Linux native runtime checks
remain outstanding; no real user configuration or cloud archives were used.

### Previous 0.2.0 Integration Checks

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
