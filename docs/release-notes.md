# MioTerm 0.0.8

MioTerm is an independent fork of ELYTerminal, extended with SSH hosts,
SFTP transfers, remote Git, system monitoring and encrypted Gitee/WebDAV sync.

Upstream: https://github.com/ELY-Open-Source/ELYTerminal

The upstream copyright and MIT license are retained. Bundled license
files are included with each package. This is not an upstream release.

## New In 0.0.8

- Private-key hosts now import selected PEM or OpenSSH identities into the
  application data directory's keys folder on Save or Quick Connect. A native
  file picker is available beside the private-key field. Original files are
  never modified; after saving, moving or removing them does not break the host.
- Repeated imports reuse content-addressed copies and include adjacent OpenSSH
  certificates. The same private key retains its remembered passphrase when its
  path changes; a different key does not inherit the previous passphrase.
  Conflicting saved credentials are not overwritten during migration.
- Cloud archives contain actual base64 key contents inside the encrypted JSON,
  not source-machine paths or separately uploaded PEM files. Restoring on another
  platform installs keys in that machine's application directory and updates
  the host paths. Edit and save older external-path hosts once to import them.
- Key installs are locked across concurrent imports/restores and use flushed
  temporary files before atomic installation. Interrupted imports can be retried.
  Existing different keys, symlinks and unsafe managed paths are not overwritten.
- Host deletion persists the connection list before removing unused credentials,
  preserving credentials referenced by other saved hosts or recent connections.
  Deletion now notifies automatic synchronization. Saving a host also notifies
  synchronization even if subsequent unused-credential cleanup fails.
- An in-progress save cannot be dismissed or replaced by another host editor.
  Matching recent connections follow imported key paths without rewriting past
  history for a different destination.

Managed keys remain ordinary OpenSSH identity files, preserving their original
key encryption format. They are protected by Unix 0700/0600 permissions or
Windows current-user ACLs, not by an additional local vault encryption layer.
Passwords and key passphrases remain in the encrypted application credential
vault. Cloud archives use the separate synchronization passphrase.

Regression coverage includes RSA PEM import, removal of the source, certificate
preservation, encrypted JSON restoration, passphrase migration/conflicts,
concurrent imports and interrupted writes. A loopback SSH/SFTP fixture exercises
real encrypted PEM authentication and upload after deleting the original key,
with SSH agents and connection reuse disabled. Live authenticated Gitee testing
was not rerun for this version. Native Windows/Linux workflows have not been
manually verified; automated platform checks run in the Release workflow.

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
