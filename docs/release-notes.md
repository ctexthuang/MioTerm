# MioTerm 0.0.2 Preview

MioTerm is an independent fork of ELYTerminal, extended with SSH hosts,
SFTP transfers, remote Git, system monitoring and encrypted Gitee/WebDAV sync.

Upstream: https://github.com/ELY-Open-Source/ELYTerminal

The upstream copyright and MIT license are retained. Bundled license
files are included with each package. This is not an upstream release.

## New In 0.0.2

- Settings > Sync selects Gitee private snippets or the existing WebDAV backend.
- Connections and actual private keys are encrypted locally with an independent
  passphrase before upload; Gitee tokens can be kept in the OS credential store.
- Saved SSH passwords and key passphrases can optionally sync in the encrypted
  archive and restore to the destination OS credential store. This is off by default.
- Gitee uses immutable private snapshots to preserve concurrent changes. History
  is not automatically deleted; old encrypted credentials may remain in history.
- Different existing credentials are never silently overwritten. Gitee snapshots
  are limited to 512 KiB; use WebDAV for larger archives.

Authenticated Gitee upload/restore, independent snapshot merge, native macOS
credential restoration, OpenSSH key verification and wrong-passphrase rejection
were tested using temporary fixtures. Test snippets and credentials were removed.

## Packages

- macOS arm64: ad-hoc signed, not Developer ID signed or Apple notarized.
- Windows x86_64: no publisher code-signing certificate.
- Linux x86_64: built on Debian 12; requires glibc 2.36 or newer.

macOS may block the downloaded preview with Gatekeeper. Windows may show
an unknown-publisher or SmartScreen warning. Only run builds you trust;
do not disable system-wide security checks.

SSH/SFTP require the system OpenSSH tools. Remote panels require Python 3;
remote Git also requires Git. Remote monitoring currently targets Linux.
Existing settings and SSH hosts remain in the ELYTerminal config folder.

These are development preview builds. Platform testing is ongoing.
SHA256SUMS.txt contains checksums for the release packages.
