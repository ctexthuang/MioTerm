use anyhow::Context as _;

use crate::term::Launch;

const BOOTSTRAP: &str = include_str!("../../assets/ssh/shell.py");

pub fn command() -> String {
    let scripts = [
        (
            "zsh/.zshenv",
            include_str!("../../assets/shell-integration/zsh/.zshenv"),
        ),
        (
            "bash/elyterminal.bash",
            include_str!("../../assets/shell-integration/bash/elyterminal.bash"),
        ),
        (
            "fish/vendor_conf.d/elyterminal.fish",
            include_str!("../../assets/shell-integration/fish/vendor_conf.d/elyterminal.fish"),
        ),
    ];
    let scripts: std::collections::BTreeMap<_, _> = scripts.into_iter().collect();
    let quote = |text: &str| format!("'{}'", text.replace('\'', "'\\''"));
    format!(
        "python3 -c {} {}",
        quote(BOOTSTRAP),
        quote(&serde_json::to_string(&scripts).expect("ssh: serializable shell scripts"))
    )
}

pub fn configure(launch: &mut Launch) -> anyhow::Result<()> {
    let (_, args) = launch.program.as_mut().context("ssh: no shell program")?;
    args.splice(
        1..1,
        [
            "-o".into(),
            "RemoteCommand=none".into(),
            "-o".into(),
            "PermitLocalCommand=no".into(),
        ],
    );
    args.push(command());
    launch.reports = true;
    log::debug!("ssh: remote shell directory reporting enabled");
    Ok(())
}

#[cfg(all(test, unix))]
mod tests;
