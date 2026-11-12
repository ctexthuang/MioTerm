# MioTerm 0.1.4

MioTerm is an independent fork of ELYTerminal, extended with SSH hosts,
SFTP transfers, remote Git, system monitoring and encrypted Gitee/WebDAV sync.

Upstream: https://github.com/ELY-Open-Source/ELYTerminal

The upstream copyright and MIT license are retained. Bundled license
files are included with each package. This is not an upstream release.

## New In 0.1.4

- Added collapsible SSH groups with host counts, keyboard activation and theme
  colors. Search results use separate collapse state, leaving the normal list's
  state intact. Collapse state stays in the current panel and is not synchronized.
- Fixed client locale variables reaching SSH servers that do not provide those
  locales. Managed SSH terminals, SFTP, remote Git and monitoring now omit
  inherited `LANG`, `LANGUAGE` and `LC_*`, including with `SendEnv *`.
- Preserved native OpenSSH configuration parsing, including relative `Include`
  paths. Local terminals retain their locale; explicit SSH `SetEnv` entries
  remain user-controlled. Open a new connection with this build to apply the fix.
- Added a standard-library-only Windows console helper, `mioterm-ssh.exe`, to
  clear locale variables after ConPTY assembles its environment. Portable ZIPs
  and per-user installers include the helper beside the main executable.
- Retained the ELYTerminal 0.2.4 baseline at `9504b53`, inline SFTP tasks, SSH
  reconnection, configurable shortcuts, local CLI control and encrypted sync.

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

Local verification passed 701 core tests, font loading, isolated application-vault,
child-process askpass and encrypted RSA PEM import tests. Transport-helper tests
covered locale removal, argument preservation, standard input/output and exit
status. Four actual loopback SSH tests covered `SendEnv *`, remote Git/SFTP,
changed transfer-path types and remote directory/terminal-version reports.
All-target Clippy with warnings denied, formatting, handwritten-file size checks
and the debug build passed. Four environment-dependent shell tests were filtered
and one cloud test stayed ignored. A Windows ConPTY regression test was added;
native Windows/Linux runtime, Windows installer execution, live Gitee and
production SSH were not verified locally. Windows typed-space diagnostics remain;
the reported black-block issue is not claimed fixed. Platform checks and
packaging run in the tag-triggered workflow.

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
