# MioTerm 0.0.9

MioTerm is an independent fork of ELYTerminal, extended with SSH hosts,
SFTP transfers, remote Git, system monitoring and encrypted Gitee/WebDAV sync.

Upstream: https://github.com/ELY-Open-Source/ELYTerminal

The upstream copyright and MIT license are retained. Bundled license
files are included with each package. This is not an upstream release.

## New In 0.0.9

- Integrated ELYTerminal 0.2.0 at upstream commit `6936b6f`, preserving MioTerm's
  SSH, SFTP, remote Git, monitoring and encrypted synchronization features.
  Upstream additions include English/Chinese UI, custom themes and backgrounds,
  session menus and reordering, Kitty graphics, clipboard permissions, packed
  scrollback, terminal performance improvements and platform fixes.
- A host without a saved SSH password no longer blocks the entire sync.
  Connection details still synchronize, with a warning about the missing secret.
  A missing selected key skips that host's local export without discarding cloud
  material or preventing other hosts from synchronizing. Corrupt vaults, invalid
  keys, conflicting credentials and network failures still report errors.
- Deleting a saved host records a persistent local exclusion. Switching views,
  automatic sync, Sync now and Restore no longer bring that host back. Excluded
  keys and credentials are not restored. Explicitly saving the host again clears
  its exclusion; distinct known private-key variants remain independent.
- Deletion protection is local to this device, not cloud deletion propagation.
  Other devices and encrypted cloud history remain intact. Hosts deleted with an
  older build must be deleted once again after upgrading. Do not run older and
  newer builds against the same configuration directory simultaneously.
- Managed SSH sessions accept streamed Kitty images but cannot read client-side
  files, temporary files or shared memory through graphics commands. Their OSC 7
  directory reports always use remote POSIX paths, including localhost reports.
- Added a Chinese manual testing guide in `docs/testing.md`, including a quiet
  Kitty graphics sample that does not insert protocol replies into the shell.

Managed keys remain ordinary OpenSSH identity files, preserving their original
key encryption format. They are protected by Unix 0700/0600 permissions or
Windows current-user ACLs, not by an additional local vault encryption layer.
Passwords and key passphrases remain in the encrypted application credential
vault. Cloud archives use the separate synchronization passphrase.

Local verification passed 518 core regression tests, all-target Clippy with
warnings denied, formatting and the handwritten-file size checks. The opt-in
live Gitee test was not run; four environment-dependent shell tests were filtered.
Isolated application-vault, askpass and RSA PEM import tests also passed. Coverage
includes repeated restore after deletion, explicit re-add, unavailable keys,
credential omission, encrypted mock WebDAV/Gitee sync and remote graphics policy.
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
