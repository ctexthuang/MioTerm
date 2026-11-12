use std::{
    env, fs,
    io::{Read, Write},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

const LOCALES: &[&str] = &["LANG", "LANGUAGE", "LC_ALL", "LC_TIME", "LC_CUSTOM"];

struct Folder(std::path::PathBuf);

impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("transport test: remove fixture");
    }
}

fn main() {
    if env::args().nth(1).as_deref() == Some("--probe") {
        for name in LOCALES {
            assert!(
                env::var_os(name).is_none(),
                "locale {name} reached the child"
            );
        }
        assert_eq!(env::var("MIOTERM_TRANSPORT_KEEP").unwrap(), "kept");
        assert_eq!(env::args().nth(2).unwrap(), "spaces '\" % ! $ \\ \u{4e2d}");
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input).unwrap();
        assert_eq!(input, "transport input\n");
        println!("transport stdio ok");
        eprintln!("transport stderr ok");
        std::process::exit(27);
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        Folder(env::temp_dir().join(format!("mioterm-transport-{}-{stamp}", std::process::id())));
    fs::create_dir(&root.0).unwrap();
    let helper = root
        .0
        .join(format!("mioterm-ssh{}", env::consts::EXE_SUFFIX));
    let output = Command::new(env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2024", "--crate-name", "mioterm_ssh"])
        .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ssh/transport.rs"))
        .arg("-o")
        .arg(&helper)
        .output()
        .expect("transport test: rustc starts");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut command = Command::new(&helper);
    command
        .arg(env::current_exe().unwrap())
        .args(["--probe", "spaces '\" % ! $ \\ \u{4e2d}"])
        .env("MIOTERM_TRANSPORT_KEEP", "kept")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for name in LOCALES {
        command.env(name, "mioterm_missing_locale");
    }
    #[cfg(windows)]
    command.env("lc_all", "mioterm_missing_locale");
    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"transport input\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(27),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"transport stdio ok\n");
    assert_eq!(output.stderr, b"transport stderr ok\n");
    let output = Command::new(&helper)
        .arg(root.0.join("missing program"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("ssh: cannot run transport:"));
    println!("ssh_transport: child locale, arguments, stdio and exit status passed");
}
