# Remote Files and Git

SSH sessions use the system OpenSSH client. On macOS and Linux, background
operations reuse the session's private control socket, including a session
authenticated with a password. The directory containing that socket is private
to the current user and removed when its session is dropped.

Open remote sessions through the app's Hosts / Quick SSH connection controls.
Typing `ssh` inside an ordinary local session does not attach remote panel
metadata or create the private control socket for that command.

## Files

The Files tab has two independently navigable panes: local above, remote below.
The address fields accept paths; Home, Parent and Refresh are icon buttons.
The remote pane initially opens the remote user's home, not the local home.
Folder activation navigates; selecting files enables upload or download.
Multiple selection uses the tree's existing keyboard and mouse controls.
Dropping local files or folders from the system file manager onto the remote
pane uploads them to its currently displayed folder, after confirmation.
The remote pane highlights valid drop targets using the current theme.
Right-click a local file or folder to upload, or a remote item to download.
Right-clicking a member of a multi-selection preserves the selected group.
Drag from the local pane to the remote pane to upload, or in reverse to download.
The destination is the opposite pane's currently open directory, including when
dropping over a listed row. All routes share the same confirmation and queue.

Transfers run through the system `sftp` client. Files and recursive folders are
supported. Every transfer asks for confirmation before writing, including the
warning that existing files may be overwritten and folders merged. Symbolic
links and special files cannot be selected for transfer. SFTP does not follow
symbolic links while recursively copying a folder. A same-name destination
which is a link, special file or a different kind is refused.

Queued tasks automatically reveal the SFTP channel and transfer list between the
panes. The channel shows the user, host and port; its tooltip explains whether
SFTP reuses the current SSH connection. The double-arrow button toggles the list
with a queue and results for the current application run. Each selected file or
recursive folder is one serial task with its status and elapsed time. Hover a
task for the original host, paths and full failure reason. The list supports cancel, retry
from the beginning, clear finished history and open the local folder. Switching
sessions or hiding the sidebar does not stop tasks; exiting the app does.
The queue holds at most 200 tasks, including finished history. Failed or cancelled
transfers may leave partial files; retry overwrites them rather than resuming.
Progress counts completed items, with an activity indicator during each item;
there is no byte percentage, speed estimate, atomic transfer or resume. Transfers
refresh both directory listings. Confirmation captures the original
files, host and destination; closing the dialog cannot erase the confirmed task.
Paths containing control characters are listed safely but refused for batch
transfer, rather than interpreted as another SFTP command. Names stay bytes in
the directory protocol; a name not representable on the local platform is
refused when downloading it.

## Reconnection

Disconnected SSH sessions offer Reconnect in the title bar and session menu.
Reconnection replaces the terminal in the same session row, retaining its name,
position and last remote folder. It does not replay commands, restore terminal
scrollback or recover a remote program; use remote tools such as tmux separately.
The Files, Git and monitoring panels attach to the new connection. Queued and
retryable transfer tasks follow that session's new control socket.

Each saved host has an optional automatic reconnect switch, off by default and
stored locally like keepalive. After a successful remote folder report, SSH exit
code 255 schedules at most five attempts, delayed 2, 5, 10, 20 and 30 seconds.
Ordinary exit, initial sign-in failures and local sessions do not retry. Detection
depends on OpenSSH and the configured keepalive interval. Automatic attempts use
strict host verification and saved credentials or noninteractive authentication.
They cannot reuse a password entered only in a terminal. Stop in the title bar
while waiting, close the session, or reconnect manually after the retry limit.

## Git

The Git tab uses the same panel for local and remote repositories. Status,
diff, history, stage, unstage, discard, commit and amend execute `git` on the
selected host. Discard keeps its existing confirmation dialog.

Managed SSH logins install the app's zsh, bash or fish integration in a private,
temporary remote folder for that session. The user's startup files are loaded,
not edited. Each prompt reports OSC 7, so `cd` updates both Files and Git.
The temporary integration is removed when the remote shell exits.
Remote Files navigation also selects the folder Git inspects; it does not type
commands into the terminal. The next terminal prompt follows its shell again.

Remote endpoints are part of an operation's identity. A queued operation keeps
its original host and repository; changing sessions does not redirect it.
Results from a previous host do not clear another host's commit draft or open a
diff in that host's view. Foreign OSC 7 reports in local terminals never open
remote paths as local folders.

## Requirements and Limits

- The client needs `ssh` and `sftp` on its executable path.
- The server needs a zsh, bash or fish login shell, Python 3 for shell
  integration and directory listing, Git for the Git tab, and an enabled SFTP
  subsystem for transfers. An unsupported shell reports an explicit error.
- Authentication can use OpenSSH config, Password, Private key, or SSH Agent.
  Password and private-key passphrase storage is opt-in via the application-owned
  encrypted file vault on all three platforms. System credential stores are used
  only by the explicit legacy import action in Settings > Sync.
- Saved credentials are supplied through an askpass helper, not arguments,
  environment variables or temporary files. Unknown host fingerprints still
  require the user's terminal confirmation. OTP/MFA prompts are not answered by
  a saved password; disable saved credentials to complete those interactively.
- Windows background operations can use saved passwords/passphrases, an agent
  or an unencrypted key. Passwords entered only in the terminal are not reused
  on Windows, which has no Unix control socket.
- Background clients always use strict host-key verification. They enable
  askpass only for an explicitly saved credential, never accept unknown hosts,
  and otherwise use BatchMode.
- Managed SSH terminals, SFTP, Git and monitoring use the server's locale.
  Their child processes omit inherited `LANG`, `LANGUAGE` and `LC_*`, including
  when SSH config uses `SendEnv *`. OpenSSH reads its own configuration normally;
  relative includes and distribution-specific system config paths keep working.
  Windows packages include a console helper, `mioterm-ssh.exe`, which clears
  these variables after ConPTY builds its environment. Local terminals retain
  their locale. Explicit SSH `SetEnv` entries remain user-controlled.
- Gitee/WebDAV sync includes authentication mode and actual selected key material.
  Explicit credential inclusion also transfers saved passwords/passphrases inside
  the encrypted archive. Keepalive and reconnect preferences remain local.
- Disconnected sessions disable new file transfers and Git actions. Refresh
  Files after finishing authentication if the initial directory read timed out.
- Directory listings are capped at 10,000 entries with an explicit error.
  Background command output is capped at 16 MiB. Directory commands time out
  after 20 seconds and Git commands after 120 seconds.

## Verification

Unit tests exercise path validation, command quoting, cancellation, bounded
processes and directory responses. On macOS, a test uses the real system SFTP
client and `sftp-server` to round-trip special filenames and recursive folders.
Native macOS UI checks against an isolated loopback SSH server cover Upload,
Download, external multi-file/folder drops, confirmation cancellation, permission
errors and directory refresh. Transferred contents are compared byte for byte.

`tests/remote_ssh.rs` is opt-in. It requires a dedicated local SSH master on
`127.0.0.1`, with `ELY_SSH_TEST_SOCKET` and `ELY_SSH_TEST_PORT` identifying it.
It modifies only a temporary test repository, exercising remote Git actions
and both SFTP directions through the authenticated connection.
