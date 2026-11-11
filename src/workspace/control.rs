//! What the main window answers a program asking through the control door.

use anyhow::{Context as _, bail};
use gpui::Context;

use super::Workspace;
use crate::control::USAGE;

impl Workspace {
    /// Answers `words`: the sessions, a session's last rows, or text or keys typed into one.
    pub fn asked(&mut self, words: &[String], cx: &mut Context<Self>) -> anyhow::Result<String> {
        let (verb, rest) = words.split_first().context("no command")?;
        if verb == "sessions" && rest.is_empty() {
            return Ok(self.listed(cx));
        }
        let (id, rest) = rest
            .split_first()
            .with_context(|| format!("{verb} of which session?\n{USAGE}"))?;
        let pid: u32 = id
            .parse()
            .with_context(|| format!("{id} is no session's number"))?;
        let session = self
            .sessions
            .iter()
            .find(|session| session.terminal.read(cx).pid() == pid)
            .with_context(|| format!("no session {pid}; `elyterminal ctl sessions` lists them"))?;
        if matches!(verb.as_str(), "send" | "key") && session.disconnected {
            bail!("ssh: session {pid} is disconnected; reconnect before sending input");
        }
        let terminal = session.terminal.clone();
        match (verb.as_str(), rest) {
            ("read", []) => Ok(terminal.read(cx).tail(None) + "\n"),
            ("read", [count]) => {
                let count = count
                    .parse()
                    .with_context(|| format!("{count} is no count of lines"))?;
                Ok(terminal.read(cx).tail(Some(count)) + "\n")
            }
            ("send", [text]) => {
                terminal.update(cx, |view, cx| view.typed(text, cx));
                Ok(String::new())
            }
            ("key", keys) if !keys.is_empty() => {
                terminal.update(cx, |view, cx| view.keyed(keys, cx))?;
                Ok(String::new())
            }
            _ => bail!("{verb} does not take that\n{USAGE}"),
        }
    }

    /// Each session on a line: its number, * for the one shown, its title and its folder, tabs between.
    fn listed(&self, cx: &Context<Self>) -> String {
        let line = |(index, session): (usize, &crate::sessions::Session)| {
            let shown = if index == self.active { "*" } else { "" };
            let pid = session.terminal.read(cx).pid();
            let folder = match &session.remote {
                Some(_) => session
                    .remote_folder
                    .as_ref()
                    .map_or_else(String::new, |path| path.shown()),
                None => crate::sessions::one_line(&session.folder.to_string_lossy()),
            };
            let connection = session.remote.as_ref().map_or_else(
                || "local".to_string(),
                |host| format!("ssh://{}", host.destination()),
            );
            let state = if session.disconnected {
                "disconnected"
            } else {
                "connected"
            };
            format!(
                "{pid}\t{shown}\t{}\t{folder}\t{}\t{state}\n",
                crate::sessions::one_line(session.named()),
                crate::sessions::one_line(&connection)
            )
        };
        self.sessions.iter().enumerate().map(line).collect()
    }
}
