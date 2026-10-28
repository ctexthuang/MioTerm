# MioTerm 0.0.3 Preview

MioTerm is an independent fork of ELYTerminal, extended with SSH hosts,
SFTP transfers, remote Git, system monitoring and encrypted Gitee/WebDAV sync.

Upstream: https://github.com/ELY-Open-Source/ELYTerminal

The upstream copyright and MIT license are retained. Bundled license
files are included with each package. This is not an upstream release.

## New In 0.0.3

- Sync encryption accepts passphrases with at least 6 characters, including
  exactly six digits. The field hint and validation error use the same minimum.
- Saving synchronization settings now explicitly prompts you to click Sync now
  to synchronize. Save does not retain or validate the encryption passphrase.
- Gitee and WebDAV upload/restore regression tests use six-digit passphrases.

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
