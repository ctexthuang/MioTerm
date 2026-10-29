# SSH Encrypted Sync

Settings > Sync selects Gitee or WebDAV. Existing WebDAV settings remain valid
and WebDAV remains the default for existing installations. Provider, vault name,
credential opt-in, automatic synchronization and secret retention preferences
are nonsecret settings. Optional remembered WebDAV passwords and encryption
passphrases use the app-owned encrypted file vault, never plaintext configuration files.
Without remembrance, those secrets must be entered for each operation.
The encryption passphrase must have at least 6 characters. Losing it means
losing access to the archive; there is no password reset or recovery key.

## Operations

- Save stores synchronization settings and explicitly remembered secrets. Secret
  fields keep their input in memory so Save can be followed by Sync now without
  retyping them. Save itself does not upload; enabled automatic sync runs afterward.
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

## Remembered Secrets And Automatic Sync

Enable Remember encryption passphrase to keep the archive passphrase in the
application credential vault. Blank fields then reuse it; a newly entered phrase
still needs confirmation. Six characters are accepted. Retention switches,
not secret values, are written to `settings.json`. Turning a retention switch
off and saving deletes the corresponding remembered secret.

Archive passphrases are scoped to the WebDAV endpoint and username, or to the
Gitee vault and token digest. Changing Gitee tokens requires entering the archive
passphrase again; a different account cannot silently reuse another account's
encryption secret. These local secrets are not synchronized to another device.
Configure the service credentials and archive passphrase there once as well.

Sync after local changes defaults off. Enabling it also enables the required
secret retention switches; enter the secrets and Save to activate it. It merges
once at application startup and after changes to saved hosts, groups or saved
SSH credentials, with a two-second debounce. Merely opening a terminal or
updating recent connection history does not trigger an upload. It does not
watch external private-key edits or periodically poll for remote-only changes.

A single application-wide worker serializes manual and automatic operations,
continues when Settings closes and does not requeue its own imported hosts.
Local edits made during a request remain pending for the next run. Failed
automatic operations remain visible in Settings and retry after 30 seconds,
2 minutes and 10 minutes, then wait for a new edit or a manual request. Missing
remembered SSH passwords stop before contacting the provider, name the affected
hosts and wait for a local change or manual retry without timed retries. The
error provides a direct Import system credentials action.
Disabling automatic sync stops queued runs, not a request already in progress.
Existing additive merge and credential-conflict rules still apply; automatic
sync is not deletion propagation or last-writer-wins replacement.

## Local File Vault

SSH passwords, private-key passphrases, remembered provider credentials and
remembered archive passphrases share an app-owned vault. No SQLite dependency
was added. Normal operations do not access macOS Keychain, Windows Credential
Manager or Linux Secret Service. The existing age library encrypts the file
using a randomly generated X25519 identity, not a hardcoded application key.

The default application data folders retain the ELYTerminal name:

- macOS: `~/Library/Application Support/ELYTerminal`
- Windows: `%APPDATA%\ELYTerminal` (the OS roaming application data folder)
- Linux: `$XDG_CONFIG_HOME/ELYTerminal`, or `~/.config/ELYTerminal`

An absolute `MIOTERM_CONFIG_DIR` overrides the application data folder for
settings, connections and credentials on all three platforms. The askpass
helper inherits it. Do not choose a source checkout or public/shared folder.

`credentials/credentials.age` holds authenticated ciphertext and
`credentials/local.key` holds its local decryption identity. `.lock` serializes
read-modify-write transactions across processes, including askpass. Writes use
restricted temporary files, flush and atomic replacement. Unix directories are
0700 and files 0600; Windows uses a protected current-user DACL. Symlinks,
Windows reparse points and Unix hard-linked files are rejected. Missing keys,
wrong keys and damaged ciphertext are errors, never a reason to reset the vault.

The identity is stored beside the ciphertext so unattended startup and automatic
sync do not need a system keychain or an unlock prompt. This is local encryption,
NOT a master-password vault: anyone who obtains both files can decrypt them.
OS user isolation and full-disk encryption remain important. Back up both files
together; never upload the local identity or this folder to a public repository.
Cloud synchronization uses the independent archive passphrase and never sends
the local identity, provider token or remembered archive passphrase.

For credentials saved by older releases, the explicit Import system credentials
button reads the original OS store and imports remembered saved/recent SSH
credentials and the Gitee token when token retention is enabled. Imports never
overwrite a different local value; the import transaction is all-or-nothing.
Original OS entries are retained and no background/native migration runs.
The result lists credentials still absent from both stores, even when other
entries imported successfully. Already available local credentials do not count
as missing. Re-enter an unavailable password in the host editor; use Restore
instead if the cloud archive already includes it. A saved connection's remembrance
flag alone does not contain the password and cannot recover one. The keyring dependency is
retained only for this opt-in upgrade operation.

For macOS to Windows, synchronize from the source with SSH credential inclusion
enabled. On Windows, configure the same provider account, vault and archive
passphrase, enable credential inclusion, then Restore. Key contents are installed
at Windows-local paths and passwords are encrypted under a fresh local identity.
No macOS keychain file, platform-specific credential blob or source path is needed.
The first device setup still needs provider access and the archive passphrase;
the encrypted archive cannot supply the secret needed to decrypt itself.

## Gitee

Create a personal access token with gists permission, select Gitee, and choose
a vault name (1-16 ASCII letters, digits, underscores or hyphens). Use the same
account, vault name and encryption passphrase on each machine. A snippet ID is
not needed. Optional token retention uses the application vault, not settings.
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
passwords and private-key passphrases are read from the application vault and
included only inside the encrypted archive. Missing remembered passwords fail
capture. Agent/default/hardware key credentials are not exported.

Credentials are associated with portable host metadata, authentication method
and key digest, not the old machine's absolute identity path. Restore writes
them to the target machine's application vault using the installed key path
and enables remembered credentials on those hosts. Credential buffers are
zeroized on drop. No credential value is written to connections or settings.

Different credentials for the same portable host stop a merge; a different
existing local credential stops restore. Local app credential writers are
serialized within a process, vault transactions are locked across processes,
and imports never replace an existing different value. Resolve
conflicting credentials explicitly; to replace an older cloud credential set,
use a new vault (or a new WebDAV folder) after checking the local entries.
Turning the switch off does not erase credentials from existing cloud archives.

## Private Keys

Every explicitly selected identity file is included in the encrypted archive,
not its original absolute path. Unreadable, missing, public-only or unsupported
identity files stop synchronization rather than producing a path-only backup.
Adjacent OpenSSH `-cert.pub` certificates are included as well.

Saving a private-key host or making a quick connection imports the selected PEM
or OpenSSH key into `keys/` under the application data directory. The source is
never changed. After a successful save, moving or deleting the source does not
affect connections or synchronization. Existing external-path hosts are imported
when edited and saved again. An unchanged key retains its saved passphrase when
its local path changes; a different key never inherits the previous passphrase.
Repeated imports reuse the same content-addressed key and include certificates.

On restore, new keys use the same application `keys/` directory, named by a digest
of their contents. Imported hosts point to those actual local files. Existing
matching local keys (including older `~/.ssh/elyterminal-sync/` keys) keep their
paths until explicitly imported again. Existing files with different
contents are never overwritten. Managed symlinks and Windows reparse points
are refused. File locks serialize simultaneous imports and restores. Each key is
flushed to a private temporary file before atomic installation; an interrupted
temporary import can be retried without exposing an incomplete identity.

The managed directory is mode 0700 and files are mode 0600 on Unix. On Windows,
protected ACLs grant access only to the current user. Restored keys are ordinary
SSH identity files, not encrypted local app-vault records. No unencrypted key
material is placed in connections JSON. Cloud JSON carries base64 key contents,
not source paths, and is encrypted as a whole before upload. If an original SSH
key has its own passphrase, it still requires that passphrase after restore.

Implicit keys from ssh_config, default identity paths, SSH agents, hardware
tokens and jump-host configuration are not exported. A key must be selected
explicitly in the saved host to be included. Hardware-backed keys cannot be
made portable by copying their on-disk reference.

## Transport And Encryption

The complete archive, including host metadata and keys, uses age passphrase
encryption through the age Rust library. Only ciphertext is sent to WebDAV.
Plaintext archive buffers and owned key data are zeroized when dropped; UI
fields are cleared when Sync now or Restore starts, not when saving settings.
Closing the settings window also discards its input. A subsequent operation
uses explicitly remembered secrets or requires input again, including retries.

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
identity derives the same public key. RSA PEM coverage includes local import,
source removal, certificate preservation, encrypted JSON restoration and saved
passphrase remapping without overwriting conflicting credentials. An optional
loopback SSH/SFTP fixture verifies real login and transfer with the imported key
after source removal, with agents and control sockets disabled.
Loopback HTTP fixtures cover conditional
GET/PUT flows, HTTP 412, authentication errors, redirects and absent ETags.
An end-to-end loopback test uploads an encrypted archive, removes the source key,
and restores the host and actual key into another machine's directory.
Gitee fixtures also exercise POST-only private snapshots, pagination, public or
truncated responses, malformed ciphertext, token rotation, independent concurrent
snapshots, authenticated history and key/password restoration. Credential
tests use isolated in-memory stores or temporary encrypted file vaults. A real Gitee
account/token is required for a live authenticated service test.

Vault tests cover restart persistence, six-character passphrase reuse, missing
keys, wrong keys, tampering, permissions, symlinks, concurrent writes, atomic
conflict rejection and independent-device archive restoration. `tests/ssh_auth`
uses an isolated `MIOTERM_CONFIG_DIR` and the real child-process askpass helper
on ordinary test runs; no user OS credentials or app vault are accessed.

The ignored `ssh::sync::gitee::live` test reads only the Gitee token saved by the
app. It uses a uniquely named temporary vault, generated encrypted SSH key and
fixture credentials, never the user's hosts or keys. It verifies authenticated
upload/restore, independent snapshots, application credential import and OpenSSH key
identity, then removes only its own snapshots and fixture vault entries. On macOS:

```sh
cargo test --release --features gpui_platform/runtime_shaders --lib ssh::sync::gitee::live:: -- --ignored --nocapture
```
