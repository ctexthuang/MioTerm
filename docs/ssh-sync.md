# SSH Encrypted Sync

Settings > Sync selects Gitee or WebDAV. Existing WebDAV settings remain valid
and WebDAV remains the default for existing installations. Provider, vault name,
credential opt-in and token retention preference are nonsecret settings.
The WebDAV password and the separate encryption
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

## Gitee

Create a personal access token with gists permission, select Gitee, and choose
a vault name (1-16 ASCII letters, digits, underscores or hyphens). Use the same
account, vault name and encryption passphrase on each machine. A snippet ID is
not needed. Optional token retention uses the OS credential store, not settings.
The token is never included in the archive and is independent of encryption.

The client uses the fixed HTTPS API origin, a sensitive Bearer authorization
header, verified certificates and no redirects. Tokens never enter URLs.
It does not follow snippet raw/download URLs. Public, incomplete, truncated,
oversized or malformed snapshots are refused. Errors do not include response
bodies, authorization headers or transport exception details.

Each upload creates a new private snippet, identified by the description
`MioTerm Sync NAME`, containing a Base64-encoded age ciphertext. The encrypted
snapshot may not exceed 512 KiB. WebDAV supports larger archives.

Gitee's documented gist API does not specify conditional atomic replacement.
The client therefore never PATCHes or DELETEs an existing snippet. Concurrent
devices create independent snapshots instead of overwriting each other. The
next synchronization merges their union. Authenticated history inside each
encrypted archive records snapshots already incorporated, allowing old snapshots
to be skipped without trusting plaintext server-provided ancestry.

Listing is bounded to ten pages of 100 snippets. A full tenth page is refused
rather than silently omitting older data. At most 32 uncovered snapshots are
decrypted per operation. Restore is read-only. History is not automatically
deleted; old snapshots may retain old encrypted credentials. Before manually
deleting history in Gitee, verify that the latest complete snapshot contains
all required data. Keep a separate backup. Use a new vault name to rotate the
encryption passphrase; existing snapshots still require their original one.

## Optional SSH Credentials

The credential switch defaults to off. When enabled, explicitly remembered SSH
passwords and private-key passphrases are read from the OS credential store and
included only inside the encrypted archive. Missing remembered passwords fail
capture. Agent/default/hardware key credentials are not exported.

Credentials are associated with portable host metadata, authentication method
and key digest, not the old machine's absolute identity path. Restore writes
them to the target machine's OS credential store using the installed key path
and enables remembered credentials on those hosts. Credential buffers are
zeroized on drop. No credential value is written to connections or settings.

Different credentials for the same portable host stop a merge; a different
existing local credential stops restore. Local app credential writers are
serialized within a process and imports never replace an existing different value. Resolve
conflicting credentials explicitly; to replace an older cloud credential set,
use a new vault (or a new WebDAV folder) after checking the local entries.
Turning the switch off does not erase credentials from existing cloud archives.

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
Gitee fixtures also exercise POST-only private snapshots, pagination, public or
truncated responses, malformed ciphertext, token rotation, independent concurrent
snapshots, authenticated history and key/password restoration. OS credential
operations use an isolated in-memory store in ordinary unit tests. A real Gitee
account/token is required for a live authenticated service test.

The ignored `ssh::sync::gitee::live` test reads only the Gitee token saved by the
app. It uses a uniquely named temporary vault, generated encrypted SSH key and
fixture credentials, never the user's hosts or keys. It verifies authenticated
upload/restore, independent snapshots, native credential import and OpenSSH key
identity, then removes only its own snapshots and OS credential entries. On macOS:

```sh
cargo test --release --features gpui_platform/runtime_shaders --lib ssh::sync::gitee::live:: -- --ignored --nocapture
```
