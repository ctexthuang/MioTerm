use std::path::Path;

use anyhow::Context as _;

use super::{Authentication, Host, auth, keys::Key};

impl Host {
    pub fn prepare(mut self, original: &Host, secret: &str) -> anyhow::Result<Self> {
        self.validate()?;
        let mut same_key = false;
        if !self.identity.is_empty() {
            let source = Path::new(&self.identity);
            let key = Key::capture(source).context("ssh: cannot import the private key")?;
            if secret.is_empty()
                && self.remember_secret
                && original.remember_secret
                && original.auth == Authentication::PrivateKey
                && self.auth == Authentication::PrivateKey
            {
                same_key = if self.identity == original.identity {
                    true
                } else if Path::new(&original.identity)
                    .try_exists()
                    .context("ssh: cannot inspect the previous private key")?
                {
                    key.same_private(
                        &Key::capture(Path::new(&original.identity))
                            .context("ssh: cannot compare the previous private key")?,
                    )
                } else {
                    false
                };
            }
            self.identity = key
                .install(&super::keys::folder()?)
                .context("ssh: cannot install the imported private key")?;
            log::info!(
                "ssh: application-managed private key ready for {}",
                self.destination()
            );
        }
        self.validate()?;
        if self.remember_secret {
            if same_key
                && auth::account(original) != auth::account(&self)
                && let Some(saved) = auth::load(original)?
            {
                auth::import(&self, &saved)
                    .context("ssh: cannot retain the imported key's saved passphrase")?;
            }
            auth::save(&self, secret)?;
        }
        Ok(self)
    }
}
