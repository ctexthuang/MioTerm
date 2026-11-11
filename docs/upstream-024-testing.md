# ELY 0.2.4 Integration Tests

The integrated upstream revision is `9504b53`. MioTerm keeps its own version and
release tags. These checks cover the new features with local and managed SSH
sessions; ordinary terminal commands that manually invoke SSH do not activate
the remote file, Git or monitoring backends.

## Run Locally

```sh
cargo run --offline --locked --release --features gpui_platform/runtime_shaders
```

The feature flag is needed only on macOS installations using runtime shaders.
Use `MIOTERM_CONFIG_DIR` with an absolute isolated directory to avoid changing
your existing settings. Only one app instance should write each directory.

## Shortcuts and Clear

1. Confirm both sidebars start closed and open only after clicking their icons.
2. Open Settings > Terminal > Shortcuts. Change Clear to an unused shortcut.
3. Try assigning Find's current key to Clear; it must report a conflict.
4. Restore or choose a free key, then print several lines in a local terminal.
5. Run Clear. The history goes away and the current prompt remains on macOS/Linux.
6. Connect a managed SSH host and repeat. Afterwards `pwd` and `ls` still work.
7. Change New session to an unused key. Confirm the new key works and the old key
   no longer starts a session. Restore the default with its reset arrow.

Windows clears scrollback while preserving ConPTY's screen. An alternate-screen
program keeps its screen when Clear runs.

## Files and Sessions

1. In a local session, open the arrow beside + and choose a folder for a session.
2. In an SSH session, choose the same action; a remote directory chooser appears.
3. Choose a remote path containing spaces or quotes and open a session there.
4. Confirm `pwd`, Files and Git refer to that host and remote path.
5. Drag a remote file into the SSH terminal. Its quoted remote path appears,
   and typing immediately appends at the terminal cursor.
6. Upload from the local pane's right-click menu; download from the remote one.
7. Drag between the local and remote panes in both directions. Confirm the
   destination and check that inline transfer tasks expand automatically.
8. Compare the uploaded/downloaded fixture contents. Hide the sidebar or switch
   sessions during a transfer; its queued task must retain the original target.

## Background and Settings

1. Select a background image and adjust its independent opacity.
2. Enable its gradient, change both colors and the angle.
3. Enable the mask with `transparent, black` and adjust size and position.
4. Toggle both sidebars and switch between local and SSH sessions.
5. Confirm text, Git/Files and monitoring remain visible and use the theme.
6. Confirm Settings > Sync still shows existing Gitee/WebDAV configuration.

The mask applies to a single picture; repeating the picture disables masking.

## Local CLI

Enable Settings > Terminal > Command line control. It is off by default.

```sh
target/release/elyterminal ctl sessions
target/release/elyterminal ctl read PID 20
target/release/elyterminal ctl send PID 'pwd'
target/release/elyterminal ctl key PID ctrl-c
```

Replace `PID` with the first column of `sessions`. The columns are process ID,
active marker, title, folder, connection and state. SSH folders are remote POSIX
paths. Use the same `MIOTERM_CONFIG_DIR` for the app and every CLI invocation
when testing with a separate data directory. On Windows pipe or redirect CLI
output, because the release executable is a window program.

1. Read and send to both a local and an SSH session; verify only that target hears it.
2. In SSH, `cd` to another folder and check the session list updates its path.
3. Print `printf '%s %s\n' "$TERM_PROGRAM" "$TERM_PROGRAM_VERSION"` in both
   shells. Both should report `mioterm` and the current Cargo version.
4. Exit the SSH session. CLI read still works; send and key fail with a
   disconnected error. Reconnecting changes its process ID; list sessions again.
5. Turn control off. New CLI requests fail and the control token file is removed.

The control token is private to this application-data directory. It is not
included in Gitee or WebDAV SSH synchronization.

## Automated Checks

```sh
cargo fmt --check
cargo clippy --offline --locked --all-targets --features gpui_platform/runtime_shaders -- -D warnings
cargo test --offline --locked --release --features gpui_platform/runtime_shaders --lib --bins --tests -- \
  --skip fish_loads_its_script_and_restores_its_data_dirs \
  --skip powershell_reports_its_folder_byte_for_byte_and_keeps_the_status \
  --skip under_wsl_each_shell_reports_its_folder_as_windows_names_it \
  --skip only_an_interactive_shell_leading_its_group_draws_its_prompt
```

The four skips apply to this macOS test host's absent shell environments; complete
CI environments should run them. Real SSH tests are opt-in as described in
[the main test guide](testing.md). Use an isolated loopback sshd and generated
keys, not production hosts. Windows NSIS installers and Linux desktop behavior
still require native checks on their respective platforms.
