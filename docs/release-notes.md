# MioTerm 0.0.5

MioTerm is an independent fork of ELYTerminal, extended with SSH hosts,
SFTP transfers, remote Git, system monitoring and encrypted Gitee/WebDAV sync.

Upstream: https://github.com/ELY-Open-Source/ELYTerminal

The upstream copyright and MIT license are retained. Bundled license
files are included with each package. This is not an upstream release.

## New In 0.0.5

- Fixed Upload and Download confirmation closing without starting a transfer.
  The confirmation now retains its transfer independently of dialog cleanup.
- Local files and folders can be dropped from the system file manager onto the
  remote Files pane, then uploaded to the displayed folder after confirmation.
- Transfer confirmation text fits the dialog; long destinations can scroll.
- Releases are ordinary GitHub releases marked Latest, rather than prereleases,
  so the repository sidebar and the app's update check can find the latest build.

Native macOS UI tests against a temporary loopback SSH server verified Upload,
Download, multi-file/folder drops, cancellation, permission errors and directory
refresh. File contents were compared byte for byte. Local regression tests,
remote Git/SFTP integration tests, Clippy, formatting and workflow lint passed.
Native drag-and-drop behavior on Windows and Linux has not been manually tested.

Authenticated Gitee upload/restore, independent snapshot merge, native macOS
credential restoration, OpenSSH key verification and wrong-passphrase rejection
were tested using temporary fixtures. Test snippets and credentials were removed.

## Packages

- macOS arm64: ad-hoc signed, not Developer ID signed or Apple notarized.
- Windows x86_64: no publisher code-signing certificate.
- Linux x86_64: built on Debian 12; requires glibc 2.36 or newer.

macOS may block the downloaded app with Gatekeeper. Windows may show
an unknown-publisher or SmartScreen warning. Only run builds you trust;
do not disable system-wide security checks.

SSH/SFTP require the system OpenSSH tools. Remote panels require Python 3;
remote Git also requires Git. Remote monitoring currently targets Linux.
Existing settings and SSH hosts remain in the ELYTerminal config folder.

MioTerm remains under active development. Platform testing is ongoing.
SHA256SUMS.txt contains checksums for the release packages.
