# MioTerm 0.1.0

MioTerm is an independent fork of ELYTerminal, extended with SSH hosts,
SFTP transfers, remote Git, system monitoring and encrypted Gitee/WebDAV sync.

Upstream: https://github.com/ELY-Open-Source/ELYTerminal

The upstream copyright and MIT license are retained. Bundled license
files are included with each package. This is not an upstream release.

## New In 0.1.0

- Integrated ELYTerminal 0.2.2 at upstream commit `ba664ef`, preserving MioTerm's
  SSH, SFTP, remote Git, monitoring and encrypted synchronization features.
  Additions since the previous MioTerm release include launch profiles, detected
  local shells, interface and CJK font settings, font weight, antialiasing,
  smooth cursor motion, cmd integration and the Touch Bar window-close fix.
- Visible SSH Files panes refresh local and remote lists and Git marks every
  second. Hidden panels pause polling; slow reads do not overlap. Selection and
  address drafts survive refreshes, and stale session results are discarded.
- The Git panel refreshes local and remote status, branches and history every
  second, coordinating reads with writes and preserving commit drafts.
- With Files menu enabled, right-click the Files icon to choose a folder for a
  new session. Local sessions use the native chooser; managed SSH sessions use
  a remote directory browser and reconnect the same host in the chosen folder.
- Each host has an SSH keepalive switch and a 1–3600 second interval. The default
  is enabled, every 30 idle seconds, with three unanswered checks before exit.
  Terminal, SFTP, Git and monitoring use the same policy on the next connection.
  This is a local transport preference and is not included in cloud archives.
- Managed SSH sessions retain the client-side graphics restrictions and remote
  POSIX directory handling. Sync retains credential omission warnings and local
  deletion exclusions; cloud deletion propagation and snapshot cleanup are not
  part of this release. Existing cloud history is preserved.
- Updated the Chinese testing guide with upstream features, remote refresh,
  folder-based sessions and keepalive checks.

Managed keys remain ordinary OpenSSH identity files, preserving their original
key encryption format. They are protected by Unix 0700/0600 permissions or
Windows current-user ACLs, not by an additional local vault encryption layer.
Passwords and key passphrases remain in the encrypted application credential
vault. Cloud archives use the separate synchronization passphrase.

Local verification passed 588 core regression tests, all-target Clippy with
warnings denied, formatting and the handwritten-file size checks. The opt-in
live Gitee test was not run; four environment-dependent shell tests were filtered.
Isolated application-vault, askpass and RSA PEM import tests also passed. Coverage
includes credential omission, encrypted mock WebDAV/Gitee sync, remote graphics
policy, refresh coordination and OpenSSH keepalive option resolution. Isolated
macOS UI checks covered remote directory sessions, local/remote refresh, Git
draft preservation, and keepalive editing, stepping and persistence.
Native Windows/Linux runtime checks were not run on the macOS development host;
automated platform checks and packaging run in the tag-triggered Release workflow.

## Packages

- macOS arm64: ad-hoc signed, not Developer ID signed or Apple notarized.
- Windows x86_64: no publisher code-signing certificate.
- Linux x86_64: tar, DEB, RPM and AppImage; built on Ubuntu 22.04 with a glibc 2.35 baseline.

macOS may block the downloaded app with Gatekeeper. Windows may show
an unknown-publisher or SmartScreen warning. Only run builds you trust;
do not disable system-wide security checks.

SSH/SFTP require the system OpenSSH tools. Remote panels require Python 3;
remote Git also requires Git. Remote monitoring currently targets Linux.
Existing settings and SSH hosts remain in the ELYTerminal config folder.

MioTerm remains under active development. Platform testing is ongoing.
SHA256SUMS.txt contains checksums for the release packages.
