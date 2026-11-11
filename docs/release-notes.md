# MioTerm 0.1.3

MioTerm is an independent fork of ELYTerminal, extended with SSH hosts,
SFTP transfers, remote Git, system monitoring and encrypted Gitee/WebDAV sync.

Upstream: https://github.com/ELY-Open-Source/ELYTerminal

The upstream copyright and MIT license are retained. Bundled license
files are included with each package. This is not an upstream release.

## New In 0.1.3

- Integrated ELYTerminal 0.2.4 at upstream commit `9504b53`, retaining MioTerm's
  SSH, SFTP, remote Git, monitoring and encrypted Gitee/WebDAV synchronization.
- Added inline SFTP tasks between the local and remote file panes. Upload or
  download through context menus, drag files between panes, or drop external
  files to upload. Confirmed tasks open the transfer area automatically and
  show their channel, destination, state and elapsed time. Queues survive panel
  and session switches, with cancellation and retry from the beginning.
- Added manual SSH reconnection and optional automatic reconnection after a
  previously established connection fails. Reconnection retains the session's
  position, name and remote directory. Normal exits do not trigger retries;
  automatic attempts are bounded and can be stopped. Reconnection does not
  recover remote processes or replay commands; transfer queues are not persisted.
- Added configurable shortcuts with recording, reset and conflict detection,
  plus terminal history clearing for local and SSH sessions. Unix retains the
  current prompt; Windows clears scrollback while preserving ConPTY's screen.
- Added optional authenticated local CLI control to list sessions, read retained
  output and send text or keys. SSH entries report remote folders and connection
  state; disconnected sessions reject input. Control is off by default and its
  local token is excluded from cloud synchronization.
- Added independent background-picture opacity, gradient colors and angle,
  plus a configurable gradient mask for a nonrepeating picture.
- Added folder selection from the profiles menu, routed to the active local or
  remote session. File-tree drops quote the path and focus the terminal. Both
  local and managed SSH shells report MioTerm's terminal name and version.
- Added per-user Windows NSIS installers for x86_64 and ARM64, alongside portable
  archives, and integrated upstream Windows input and PowerShell policy changes.
- Preserved the existing application data directories, default closed sidebars,
  theme colors, Ely desktop character and animated session status marks.

Upgrade every device before sharing the new record vault. Older binaries cannot
read the new schema; do not use them against a migrated local data folder. The
existing ELYTerminal settings directories are retained.

Deletion markers retain IDs only, with a 10,000-marker limit. Gitee archives are
limited to 512 KiB of encrypted data. Gitee does not provide atomic conditional
replacement: preflight and readback checks detect some concurrent writes but
cannot eliminate the GET-to-PATCH race. WebDAV with strong ETags supports atomic
conditional writes. See docs/ssh-sync.md for migration and storage details.

Managed keys remain ordinary OpenSSH identity files, preserving their original
key encryption format. They are protected by Unix 0700/0600 permissions or
Windows current-user ACLs, not by an additional local vault encryption layer.
Passwords and key passphrases remain in the encrypted application credential
vault. Cloud archives use the separate synchronization passphrase.

Local integration verification passed 698 core tests, font loading, isolated
application-vault, child-process askpass and encrypted RSA PEM import tests.
Three actual loopback SSH tests covered remote Git/SFTP, changed transfer-path
types, and remote directory/terminal-version reports. Native macOS checks covered
custom shortcuts, local and SSH CLI operations, disconnected-input refusal,
SSH history clearing, quoted-path dragging and inline SFTP upload completion.
All-target Clippy with warnings denied, formatting, handwritten-file size checks
and the release build passed. Four environment-dependent shell tests were
filtered and one cloud test stayed ignored. Live Gitee, production SSH, native
Windows/Linux runtime and Windows installer execution were not verified locally.
Windows typed-space diagnostics are included; the reported black-block issue is
not claimed fixed. Platform checks and packaging run in the tag-triggered workflow.

## Packages

- macOS arm64: ad-hoc signed, not Developer ID signed or Apple notarized.
- Windows x86_64 and ARM64: no publisher code-signing certificate.
- Linux x86_64: tar, DEB, RPM and AppImage; built on Ubuntu 22.04 with a glibc 2.35 baseline.

macOS may block the downloaded app with Gatekeeper. Windows may show
an unknown-publisher or SmartScreen warning. Only run builds you trust;
do not disable system-wide security checks.

SSH/SFTP require the system OpenSSH tools. Remote panels require Python 3;
remote Git also requires Git. Remote monitoring currently targets Linux.
Existing settings and SSH hosts remain in the ELYTerminal config folder.

MioTerm remains under active development. Platform testing is ongoing.
SHA256SUMS.txt contains checksums for the release packages.
