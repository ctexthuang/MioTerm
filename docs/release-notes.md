# MioTerm 0.1.2

MioTerm is an independent fork of ELYTerminal, extended with SSH hosts,
SFTP transfers, remote Git, system monitoring and encrypted Gitee/WebDAV sync.

Upstream: https://github.com/ELY-Open-Source/ELYTerminal

The upstream copyright and MIT license are retained. Bundled license
files are included with each package. This is not an upstream release.

## New In 0.1.2

- Reworked SSH synchronization around stable record identities, logical revisions
  and synced deletion markers, following Termora's record model. Renaming or
  regrouping a host updates the same record; deleted hosts no longer return from
  an offline device. Identical legacy profiles and unambiguous ungrouped copies
  are deduplicated while distinct profiles and keys remain separate.
- Gitee now creates one private record snippet and updates that snippet on later
  syncs. Legacy snapshots are consolidated once when uploading the new format;
  later syncs do not reread them. Old snippets and provider revision history
  remain available. The current archive prunes deleted hosts and unused secrets.
- Added Manual (default) and On change sync policies. Saved host or credential
  changes schedule synchronization after a random 10-29 second delay; subsequent
  edits replace that deadline. Startup and saving sync settings do not initiate
  sync. Sync now or Restore receives remote changes when no local edit occurs.
  Successful operations display and persist their completion time.
- Sync operations share one worker, pull and merge before upload, and preserve
  edits made while a request is running. Failed downloads or decryption stop the
  upload. Missing credentials do not block unrelated hosts.
- Fixed shared password/passphrase updates not advancing peer record revisions,
  recent connections retaining stale authentication, legacy deletions returning
  when a key is missing, and rejected sync results writing credentials too early.
- Retained ELYTerminal 0.2.3 at upstream commit `fdca49a`, SSH, SFTP, remote Git,
  monitoring, local file vaults and cross-platform private-key content transfer.

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

Local verification passed 649 core tests, including 90 synchronization tests,
all-target Clippy with warnings denied, formatting and handwritten-file size
checks. Seven new review regressions cover credential and concurrent-edit cases.
Isolated application-vault, child-process askpass and RSA PEM import tests passed.
macOS UI checks covered both sync policies, completion time layout, and no startup
sync with On change enabled. Four environment-dependent shell tests were filtered;
live Gitee, production SSH and native Windows/Linux runtime checks were not run
for this release. Automated platform checks and packaging run in the tag-triggered
Release workflow.

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
