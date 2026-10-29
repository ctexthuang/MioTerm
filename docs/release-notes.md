# MioTerm 0.0.6

MioTerm is an independent fork of ELYTerminal, extended with SSH hosts,
SFTP transfers, remote Git, system monitoring and encrypted Gitee/WebDAV sync.

Upstream: https://github.com/ELY-Open-Source/ELYTerminal

The upstream copyright and MIT license are retained. Bundled license
files are included with each package. This is not an upstream release.

## New In 0.0.6

- SSH passwords, private-key passphrases and remembered sync secrets now use an
  application-owned encrypted file vault on macOS, Windows and Linux. No SQLite
  dependency was added. Normal operations no longer require a system keychain.
- Import system credentials explicitly migrates older SSH credentials and saved
  Gitee tokens. Original system entries are retained; conflicts are not overwritten.
  The result identifies credentials that are still missing.
- Encryption passphrases of exactly six characters can be remembered and reused
  after restart. Save retains typed secrets instead of clearing unsaved input.
- Optional automatic sync runs at startup and after saved host, group or credential
  changes, with a two-second debounce and one shared manual/automatic worker.
- Missing remembered SSH passwords identify affected hosts, offer a direct import
  action and stop before contacting the provider. They wait for a local change or
  manual retry instead of repeatedly retrying on a timer. Restore can recover a
  password already present in the encrypted cloud archive.
- An absolute MIOTERM_CONFIG_DIR selects the application data folder on all three
  platforms, including the child-process SSH askpass helper.

The local vault stores credentials.age beside a randomly generated local.key.
Unix permissions and Windows current-user ACLs restrict access, but possession of
both files permits decryption. This is not a master-password vault. Cloud archives
use the separate user passphrase and never include the local key or sync tokens.

For macOS-to-Windows migration, first include SSH credentials when syncing from
the source. Configure the same provider account, vault and encryption passphrase
on Windows, enable credential inclusion and use Restore. Actual key contents and
passwords are restored using Windows-local paths and fresh local vault encryption.

Local regression tests cover restart persistence, concurrent file writers, the
actual child-process askpass helper, atomic import conflicts, missing-credential
preflight and read-only password restoration from a loopback cloud archive.
Native macOS UI checks verified Save/reuse and the missing-password import action.
Live authenticated Gitee testing was not rerun for this version, and the new native
Windows/Linux user workflows have not been manually verified.

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
