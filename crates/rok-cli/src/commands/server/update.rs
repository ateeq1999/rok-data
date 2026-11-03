//! `rok server update <name>`: change a registered server's settings, its
//! password, or both (ADR 0028).

use super::{config_from_url, prompt_password, url_password};
use crate::config::{read_servers, servers_path, write_servers};
use crate::keychain::{KeychainError, env_var_name, store_password};
use crate::output::{json_line, server_update_json};

/// The update to make; every setting left `None` keeps its value.
pub(super) struct ServerUpdate {
    pub(super) name: String,
    pub(super) url: Option<String>,
    pub(super) host: Option<String>,
    pub(super) port: Option<u16>,
    pub(super) user: Option<String>,
    pub(super) database: Option<String>,
    pub(super) sslmode: Option<String>,
    pub(super) password_prompt: bool,
    pub(super) json: bool,
}

impl ServerUpdate {
    pub(super) fn run(&self) -> Result<std::process::ExitCode, String> {
        let mut config = read_servers(None)?;
        let server = config
            .servers
            .iter_mut()
            .find(|server| server.name == self.name)
            .ok_or_else(|| {
                format!(
                    "server '{}' not found; `rok server add {}` registers it",
                    self.name, self.name
                )
            })?;

        // The settings first: a URL replaces the connection whole.
        let mut changed: Vec<&str> = Vec::new();
        if let Some(url) = &self.url {
            *server = config_from_url(&self.name, url)?;
            changed.extend(["host", "port", "user", "database", "sslmode"]);
        }
        if let Some(host) = &self.host {
            server.host.clone_from(host);
            changed.push("host");
        }
        if let Some(port) = self.port {
            server.port = port;
            changed.push("port");
        }
        if let Some(user) = &self.user {
            server.user.clone_from(user);
            changed.push("user");
        }
        if let Some(database) = &self.database {
            server.database = Some(database.clone());
            changed.push("database");
        }
        if let Some(sslmode) = &self.sslmode {
            server.sslmode = Some(sslmode.clone());
            changed.push("sslmode");
        }

        // Then the password: from the URL, else the prompt, else unchanged.
        let password = match &self.url {
            Some(url) => url_password(url)?,
            None => None,
        };
        let password = match password {
            Some(password) => Some(password),
            None if self.password_prompt => Some(prompt_password()?),
            None => None,
        };
        let stored = match &password {
            None => None,
            Some(password) => match store_password(&self.name, password) {
                Ok(()) => Some(true),
                Err(KeychainError::Unavailable(message)) => {
                    if !self.json {
                        eprintln!("warning: {message}");
                        eprintln!(
                            "Set {} in the environment to provide the password.",
                            env_var_name(&self.name)
                        );
                    }
                    Some(false)
                }
                Err(err) => return Err(err.to_string()),
            },
        };

        if !changed.is_empty() {
            write_servers(&servers_path()?, &config)?;
        }

        if self.json {
            json_line(server_update_json(&self.name, &changed, stored));
        } else {
            let mut said = Vec::new();
            if !changed.is_empty() {
                said.push(changed.join(", "));
            }
            match stored {
                Some(true) => said.push("password stored in OS keychain".to_owned()),
                Some(false) => said.push(format!("password from {}", env_var_name(&self.name))),
                None => {}
            }
            println!("Server '{}' updated: {}.", self.name, said.join("; "));
        }
        Ok(std::process::ExitCode::SUCCESS)
    }
}
