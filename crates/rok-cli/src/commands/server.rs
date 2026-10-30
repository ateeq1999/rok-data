//! `rok server` command group (ADR 0028).
//!
//! Per-user server registry with OS keychain password storage.

use crate::cli::ServerAction;
use crate::config::{ServerConfig, read_servers, servers_path, write_servers};
use crate::keychain::{KeychainError, delete_password, env_var_name, get_password, store_password};
use crate::output::{PasswordStatus, server_add_json, server_list_json, server_remove_json};
use std::io::Write;

/// Runs the `server` subcommand.
pub fn run(action: ServerAction) -> Result<std::process::ExitCode, String> {
    match action {
        ServerAction::Add {
            name,
            url,
            host,
            port,
            user,
            database,
            sslmode,
            password_prompt,
            json,
        } => {
            let add = ServerAdd {
                name,
                url,
                host,
                port,
                user,
                database,
                sslmode,
                password_prompt,
                json,
            };
            add.run()
        }
        ServerAction::List { json } => list(json).map(|_| std::process::ExitCode::SUCCESS),
        ServerAction::Remove { name, yes, json } => {
            let remove = ServerRemove { name, yes, json };
            remove.run()
        }
    }
}

struct ServerAdd {
    name: String,
    url: Option<String>,
    host: Option<String>,
    port: u16,
    user: Option<String>,
    database: Option<String>,
    sslmode: Option<String>,
    password_prompt: bool,
    json: bool,
}

impl ServerAdd {
    fn run(&self) -> Result<std::process::ExitCode, String> {
        // Validate and build server config
        let server = self.build_server_config()?;

        // Check if server already exists
        let mut config = read_servers(None)?;
        if config.servers.iter().any(|s| s.name == server.name) {
            return Err(format!("server '{}' already exists", server.name));
        }

        // Extract password from URL or prompt
        let password = self.extract_password()?;

        // Try to store password in keychain
        let keychain_available = match store_password(&server.name, &password) {
            Ok(()) => true,
            Err(KeychainError::Unavailable(msg)) => {
                // Keychain unavailable - warn and instruct user to use env var
                if !self.json {
                    eprintln!("warning: {msg}");
                    eprintln!(
                        "Set {} in the environment to provide the password.",
                        env_var_name(&server.name)
                    );
                }
                false
            }
            Err(e) => return Err(e.to_string()),
        };

        // Add server to config (without password)
        config.servers.push(server.clone());
        write_servers(&servers_path()?, &config)?;

        // Success message
        if self.json {
            let json = server_add_json(&server.name, keychain_available);
            crate::output::json_line(json);
        } else if keychain_available {
            println!(
                "Server '{}' added. Password stored in OS keychain (service: rok, account: {}).",
                server.name, server.name
            );
        } else {
            println!(
                "Server '{}' added (password from {}).",
                server.name,
                env_var_name(&server.name)
            );
        }

        Ok(std::process::ExitCode::SUCCESS)
    }

    fn build_server_config(&self) -> Result<ServerConfig, String> {
        if let Some(ref url) = self.url {
            self.parse_url(url)
        } else {
            self.build_from_flags()
        }
    }

    fn parse_url(&self, url: &str) -> Result<ServerConfig, String> {
        // Parse postgres:// URL
        let parsed = url::Url::parse(url).map_err(|e| format!("invalid URL: {e}"))?;

        if parsed.scheme() != "postgres" && parsed.scheme() != "postgresql" {
            return Err("URL scheme must be postgres:// or postgresql://".to_owned());
        }

        let host = parsed
            .host_str()
            .ok_or("URL must include a host")?
            .to_owned();
        let port = parsed.port().unwrap_or(5432);
        let user = parsed.username().to_owned();
        if user.is_empty() {
            return Err("URL must include a username".to_owned());
        }

        // Database from path (first segment)
        let path_segments: Vec<&str> = parsed
            .path_segments()
            .map(|s| s.collect())
            .unwrap_or_default();
        let database = path_segments.first().map(|s| s.to_string());

        // sslmode from query
        let sslmode = parsed
            .query_pairs()
            .find(|(k, _)| k == "sslmode")
            .map(|(_, v)| v.to_string());

        Ok(ServerConfig {
            name: self.name.clone(),
            host,
            port,
            user,
            database,
            sslmode,
        })
    }

    fn build_from_flags(&self) -> Result<ServerConfig, String> {
        let host = self
            .host
            .clone()
            .ok_or("--host is required when not using --url")?;
        let user = self
            .user
            .clone()
            .ok_or("--user is required when not using --url")?;

        Ok(ServerConfig {
            name: self.name.clone(),
            host,
            port: self.port,
            user,
            database: self.database.clone(),
            sslmode: self.sslmode.clone(),
        })
    }

    fn extract_password(&self) -> Result<String, String> {
        if let Some(ref url) = self.url {
            // Try to extract from URL
            let parsed = url::Url::parse(url).map_err(|e| format!("invalid URL: {e}"))?;
            if let Some(pwd) = parsed.password() {
                return Ok(pwd.to_owned());
            }
        }

        if self.password_prompt {
            return prompt_password();
        }

        // No password provided
        Err(
            "password required: provide via --url with password, or use --password-prompt"
                .to_owned(),
        )
    }
}

struct ServerRemove {
    name: String,
    yes: bool,
    json: bool,
}

impl ServerRemove {
    fn run(&self) -> Result<std::process::ExitCode, String> {
        let mut config = read_servers(None)?;

        // Find and remove server
        let pos = config
            .servers
            .iter()
            .position(|s| s.name == self.name)
            .ok_or_else(|| format!("server '{}' not found", self.name))?;

        // Confirm before removing (unless --yes)
        if !self.yes && !self.json {
            print!("Type the server name to confirm: ");
            std::io::stdout().flush().map_err(|e| e.to_string())?;
            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .map_err(|e| e.to_string())?;
            let input = input.trim();
            if input != self.name {
                return Err("confirmation failed: server name does not match".to_owned());
            }
        }

        config.servers.remove(pos);
        write_servers(&servers_path()?, &config)?;

        // Delete password from keychain
        let keychain_available = match delete_password(&self.name) {
            Ok(()) => true,
            Err(KeychainError::Unavailable(msg)) => {
                if !self.json {
                    eprintln!("warning: {msg}");
                    eprintln!("Unset {} if no longer needed.", env_var_name(&self.name));
                }
                false
            }
            Err(e) => return Err(e.to_string()),
        };

        // Success message
        if self.json {
            let json = server_remove_json(&self.name, keychain_available);
            crate::output::json_line(json);
        } else if keychain_available {
            println!(
                "Server '{}' removed. Password deleted from OS keychain.",
                self.name
            );
        } else {
            println!(
                "Server '{}' removed. Unset {} if no longer needed.",
                self.name,
                env_var_name(&self.name)
            );
        }

        Ok(std::process::ExitCode::SUCCESS)
    }
}

fn list(json: bool) -> Result<(), String> {
    let config = read_servers(None)?;

    if config.servers.is_empty() {
        if json {
            let json = server_list_json(&[], &[]);
            crate::output::json_line(json);
        } else {
            println!("No servers registered.");
        }
        return Ok(());
    }

    // Check password status for each server
    let mut password_statuses = Vec::new();
    for server in &config.servers {
        let status = match get_password(&server.name) {
            Ok(_) => {
                // Check if it came from keychain or env
                let env_var = env_var_name(&server.name);
                if std::env::var(&env_var).is_ok() {
                    PasswordStatus::Env
                } else {
                    PasswordStatus::Keychain
                }
            }
            Err(_) => PasswordStatus::Missing,
        };
        password_statuses.push(status);
    }

    if json {
        let json = server_list_json(&config.servers, &password_statuses);
        crate::output::json_line(json);
    } else {
        // Print table header
        println!(
            "{:<12} {:<30} {:>5} {:<12} {:<12} PASSWORD",
            "NAME", "HOST", "PORT", "USER", "DEFAULT DB"
        );
        println!("{}", "-".repeat(90));

        for (server, status) in config.servers.iter().zip(password_statuses.iter()) {
            let password_str = match status {
                PasswordStatus::Keychain => "keychain ✓".to_owned(),
                PasswordStatus::Env => {
                    let env_var = env_var_name(&server.name);
                    format!("env {} ✓", env_var)
                }
                PasswordStatus::Missing => "missing ✗".to_owned(),
            };

            let db = server.database.as_deref().unwrap_or("-");
            println!(
                "{:<12} {:<30} {:>5} {:<12} {:<12} {}",
                server.name, server.host, server.port, server.user, db, password_str
            );
        }
    }

    Ok(())
}

fn prompt_password() -> Result<String, String> {
    use std::io::{stdin, stdout};

    print!("Enter password: ");
    stdout().flush().map_err(|e| e.to_string())?;

    let mut password = String::new();
    stdin()
        .read_line(&mut password)
        .map_err(|e| e.to_string())?;
    Ok(password.trim_end().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_add_parses_url() {
        let add = ServerAdd {
            name: "local".to_owned(),
            url: Some(
                "postgres://postgres:secret@localhost:5432/postgres?sslmode=prefer".to_owned(),
            ),
            host: None,
            port: 5432,
            user: None,
            database: None,
            sslmode: None,
            password_prompt: false,
            json: false,
        };
        let server = add.parse_url(add.url.as_ref().unwrap()).unwrap();
        assert_eq!(server.name, "local");
        assert_eq!(server.host, "localhost");
        assert_eq!(server.port, 5432);
        assert_eq!(server.user, "postgres");
        assert_eq!(server.database, Some("postgres".to_owned()));
        assert_eq!(server.sslmode, Some("prefer".to_owned()));
    }

    #[test]
    fn server_add_parses_url_without_password() {
        let add = ServerAdd {
            name: "ci".to_owned(),
            url: Some("postgres://user@host/db".to_owned()),
            host: None,
            port: 5432,
            user: None,
            database: None,
            sslmode: None,
            password_prompt: false,
            json: false,
        };
        let server = add.parse_url(add.url.as_ref().unwrap()).unwrap();
        assert_eq!(server.name, "ci");
        assert_eq!(server.host, "host");
        assert_eq!(server.user, "user");
        assert_eq!(server.database, Some("db".to_owned()));
    }

    #[test]
    fn server_add_builds_from_flags() {
        let add = ServerAdd {
            name: "staging".to_owned(),
            url: None,
            host: Some("staging-db.example.com".to_owned()),
            port: 5432,
            user: Some("rok".to_owned()),
            database: Some("postgres".to_owned()),
            sslmode: Some("require".to_owned()),
            password_prompt: false,
            json: false,
        };
        let server = add.build_from_flags().unwrap();
        assert_eq!(server.name, "staging");
        assert_eq!(server.host, "staging-db.example.com");
        assert_eq!(server.user, "rok");
        assert_eq!(server.sslmode, Some("require".to_owned()));
    }

    #[test]
    fn server_add_requires_host_and_user_without_url() {
        let add = ServerAdd {
            name: "test".to_owned(),
            url: None,
            host: None,
            port: 5432,
            user: None,
            database: None,
            sslmode: None,
            password_prompt: false,
            json: false,
        };
        let result = add.build_from_flags();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("--host is required"));
    }
}
