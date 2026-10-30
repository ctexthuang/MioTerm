use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn a_duplicate_remote_shell_opens_in_its_reported_folder() {
    let root = std::env::temp_dir().join(format!("ely-remote-duplicate-{}", std::process::id()));
    let folder = root.join("space ' ; $(never-run)");
    fs::create_dir_all(&folder).unwrap();
    let folder = folder.canonicalize().unwrap();
    let path = super::super::files::RemotePath::from_path(&folder);
    let command = super::command_in(Some(&path));
    let mut child = Command::new("/bin/sh")
        .args(["-c", &command])
        .env("SHELL", "/bin/bash")
        .env("HOME", &root)
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"printf 'DUPLICATE:%s\\n' \"$PWD\"\nexit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains(&format!("DUPLICATE:{}", folder.display())),
        "{:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn remote_shell_reports_each_directory_without_changing_startup_files() {
    let root = std::env::temp_dir().join(format!("ely-remote-shell-{}", std::process::id()));
    fs::create_dir_all(root.join("second % directory")).unwrap();
    for shell in ["/bin/zsh", "/bin/bash"] {
        if !std::path::Path::new(shell).exists() {
            continue;
        }
        let home = root.join(if shell.ends_with("zsh") {
            "zsh"
        } else {
            "bash"
        });
        fs::create_dir_all(&home).unwrap();
        fs::write(home.join(".zshrc"), "PS1='fixture> '\n").unwrap();
        fs::write(home.join(".bash_profile"), "PS1='fixture> '\n").unwrap();
        let mut child = Command::new("/bin/sh")
            .args(["-c", &super::command()])
            .env("SHELL", shell)
            .env("HOME", &home)
            .env_remove("ZDOTDIR")
            .env_remove("ELY_ZSH_ZDOTDIR")
            .current_dir(&root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"cd 'second % directory'\nfalse\nprintf 'status:%s\\n' \"$?\"\nexit\n")
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = String::from_utf8_lossy(&output.stdout);
        assert!(output.contains("\x1b]7;file://"), "{shell}: {output:?}");
        assert!(
            output.contains("/second %25 directory\x07"),
            "{shell}: {output:?}"
        );
        assert!(output.contains("status:1"), "{shell}: {output:?}");
        assert_eq!(
            fs::read_to_string(home.join(".zshrc")).unwrap(),
            "PS1='fixture> '\n"
        );
    }
    fs::remove_dir_all(root).unwrap();
}
