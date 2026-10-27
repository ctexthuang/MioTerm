# SSH WebDAV Sync

Settings > WebDAV Sync accepts an existing HTTPS WebDAV folder and a username.
Only those two fields are saved. The WebDAV password and the separate encryption
passphrase are entered for each operation and are never written to settings.
The encryption passphrase must have at least 12 characters. Losing it means
losing access to the archive; there is no password reset or recovery key.

## Operations

- Sync now downloads and decrypts the existing archive, merges saved hosts,
  encrypts the combined archive, conditionally uploads it, and imports the merged
  hosts locally.
- Restore downloads and merges the archive locally without changing the server.
  It can repair a connection whose old identity path no longer exists.
- Exact duplicates are removed. Different names, users, ports, groups or key
  contents are retained as distinct entries. This is an additive merge, not
  automatic deletion propagation or last-writer-wins replacement.
- Recent connection history, terminal contents, settings and known_hosts are
  not synchronized.

## Private Keys

Every explicitly selected identity file is included in the encrypted archive,
not its original absolute path. Unreadable, missing, public-only or unsupported
identity files stop synchronization rather than producing a path-only backup.
Adjacent OpenSSH `-cert.pub` certificates are included as well.

On restore, keys are written under `~/.ssh/elyterminal-sync/`, named by a digest
of their contents. Imported hosts point to those actual local files. Existing
matching local keys keep their original paths. Existing files with different
contents are never overwritten. Managed symlinks and Windows reparse points
are refused.

The managed directory is mode 0700 and files are mode 0600 on Unix. On Windows,
protected ACLs grant access only to the current user. Restored keys are ordinary
SSH identity files, not encrypted local app-vault records. If an original SSH
key has its own passphrase, it still requires that passphrase after restore.

Implicit keys from ssh_config, default identity paths, SSH agents, hardware
tokens and jump-host configuration are not exported. A key must be selected
explicitly in the saved host to be included. Hardware-backed keys cannot be
made portable by copying their on-disk reference.

## Transport And Encryption

The complete archive, including host metadata and keys, uses age passphrase
encryption through the age Rust library. Only ciphertext is sent to WebDAV.
Plaintext archive buffers and owned key data are zeroized when dropped; UI
fields are cleared when an operation starts.

HTTPS certificates are verified. Embedded URL credentials, query tokens,
fragments, plain HTTP and redirects are rejected. Authorization headers are
marked sensitive. HTTP errors report status codes without server response
bodies or passwords.

First uploads use `If-None-Match: *`; updates require the strong ETag obtained
by the preceding GET and use `If-Match`. HTTP 412 stops the operation for retry.
Servers without strong ETags can be restored from, but their existing archives
are never overwritten. Conditional writes depend on the WebDAV server honoring
HTTP preconditions correctly.

The app bounds archive, key and record sizes, limits scrypt work on incoming
archives, and uses network timeouts. If local hosts change while an operation
is running, local application is refused for retry. An upload may already have
completed, but the newer local edits are not overwritten.

## Verification

Unit tests cover encrypted round trips, wrong passphrases, ciphertext tampering,
key restoration after removal of the source, restrictive permissions, symlink
refusal, duplicate merging, key digests and concurrent local edits. An OpenSSH
test generates a temporary passphrase-protected key and checks that the restored
identity derives the same public key. Loopback HTTP fixtures cover conditional
GET/PUT flows, HTTP 412, authentication errors, redirects and absent ETags.
An end-to-end loopback test uploads an encrypted archive, removes the source key,
and restores the host and actual key into another machine's directory.
