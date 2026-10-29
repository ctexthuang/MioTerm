use std::io::{BufRead, Read, Write};

use anyhow::{Context as _, ensure};

fn accepts(kind: &str, prompt: &str, hint: &str) -> bool {
    if !hint.is_empty() {
        return false;
    }
    let prompt = prompt.to_ascii_lowercase();
    match kind {
        "password" => prompt.trim_end().ends_with("password:"),
        "private_key" => prompt.starts_with("enter passphrase for key "),
        _ => false,
    }
}

pub fn run() -> Option<i32> {
    let account = std::env::var("ELY_SSH_ASKPASS_ACCOUNT").ok()?;
    let result = answer(&account);
    if let Err(error) = &result {
        eprintln!("{error:#}");
    }
    Some(if result.is_ok() { 0 } else { 1 })
}

fn answer(account: &str) -> anyhow::Result<()> {
    ensure!(
        account.len() == 64 && account.bytes().all(|c| c.is_ascii_hexdigit()),
        "ssh: invalid credential identifier"
    );
    let prompt = std::env::args()
        .nth(1)
        .context("ssh: askpass prompt missing")?;
    let hint = std::env::var("SSH_ASKPASS_PROMPT").unwrap_or_default();
    let kind = std::env::var("ELY_SSH_ASKPASS_KIND").unwrap_or_default();
    let answer = if accepts(&kind, &prompt, &hint) {
        super::load_account(account)?
            .context("ssh: saved credential unavailable; edit the host or import system credentials in Settings > Sync")?
    } else {
        let confirm = hint == "confirm"
            || prompt.contains("Are you sure you want to continue connecting")
            || prompt.starts_with("Please type 'yes'");
        terminal_answer(&prompt, confirm)?
    };
    std::io::stdout()
        .write_all(answer.as_bytes())
        .context("ssh: cannot send askpass response")?;
    std::io::stdout().write_all(b"\n")?;
    Ok(())
}

fn terminal_answer(prompt: &str, confirm: bool) -> anyhow::Result<zeroize::Zeroizing<String>> {
    ensure!(
        confirm,
        "ssh: additional authentication is required; disable saved credentials to answer it in the terminal"
    );
    #[cfg(unix)]
    let (input, output) = (
        std::fs::File::open("/dev/tty"),
        std::fs::OpenOptions::new().write(true).open("/dev/tty"),
    );
    #[cfg(windows)]
    let (input, output) = (
        std::fs::File::open("CONIN$"),
        std::fs::OpenOptions::new().write(true).open("CONOUT$"),
    );
    let input = input.context("ssh: host key confirmation needs a terminal")?;
    let mut output = output.context("ssh: host key confirmation needs a terminal")?;
    write!(output, "{prompt} ")?;
    output.flush()?;
    let mut line = zeroize::Zeroizing::new(String::new());
    std::io::BufReader::new(input)
        .take(4096)
        .read_line(&mut line)?;
    let length = line.trim_end_matches(['\r', '\n']).len();
    line.truncate(length);
    ensure!(!line.is_empty(), "ssh: confirmation cancelled");
    Ok(line)
}

#[cfg(test)]
mod tests {
    use super::accepts;

    #[test]
    fn stored_passwords_never_answer_host_trust_or_other_challenges() {
        assert!(accepts("password", "root@example's password: ", ""));
        assert!(accepts(
            "private_key",
            "Enter passphrase for key '/keys/id': ",
            ""
        ));
        for (kind, prompt, hint) in [
            ("password", "root@example's password:", "confirm"),
            ("password", "Verification code:", ""),
            ("password", "Enter passphrase for key '/keys/id':", ""),
            ("private_key", "root@example's password:", ""),
            ("agent", "Password:", ""),
        ] {
            assert!(!accepts(kind, prompt, hint));
        }
    }
}
