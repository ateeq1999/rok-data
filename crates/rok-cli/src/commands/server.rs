//! `rok server` command group (ADR 0028).
//!
//! Per-user server registry with OS keychain password storage.

use crate::cli::ServerAction;
use crate::config::{ServerConfig, read_servers, servers_path, write_servers};
use crate::keychain::{KeychainError, delete_password, env_var_name, get_password, store_password};
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
            };
            add.run()
        }
        ServerAction::List => list().map(|_| std::process::ExitCode::SUCCESS),
        ServerAction::Remove { name } => {
            let remove = ServerRemove { name };
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
                eprintln!("warning: {msg}");
                eprintln!(
                    "Set {} in the environment to provide the password.",
                    env_var_name(&server.name)
                );
                false
            }
            Err(e) => return Err(e.to_string()),
        };

        // Add server to config (without password)
        config.servers.push(server.clone());
        write_servers(&servers_path()?, &config)?;

        // Success message
        if keychain_available {
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

        config.servers.remove(pos);
        write_servers(&servers_path()?, &config)?;

        // Delete password from keychain
        match delete_password(&self.name) {
            Ok(()) => {
                println!(
                    "Server '{}' removed. Password deleted from OS keychain.",
                    self.name
                );
            }
            Err(KeychainError::Unavailable(msg)) => {
                eprintln!("warning: {msg}");
                eprintln!("Unset {} if no longer needed.", env_var_name(&self.name));
            }
            Err(e) => return Err(e.to_string()),
        }

        Ok(std::process::ExitCode::SUCCESS)
    }
}

fn list() -> Result<(), String> {
    let config = read_servers(None)?;

    if config.servers.is_empty() {
        println!("No servers registered.");
        return Ok(());
    }

    // Print table header
    println!(
        "{:<12} {:<30} {:>5} {:<12} {:<12} PASSWORD",
        "NAME", "HOST", "PORT", "USER", "DEFAULT DB"
    );
    println!("{}", "-".repeat(90));

    for server in &config.servers {
        // Check if password exists in keychain or env
        let password_status = match get_password(&server.name) {
            Ok(_) => {
                // Check if it came from keychain or env
                let env_var = env_var_name(&server.name);
                if std::env::var(&env_var).is_ok() {
                    format!("env {} ✓", env_var)
                } else {
                    "keychain ✓".to_owned()
                }
            }
            Err(_) => "missing ✗".to_owned(),
        };

        let db = server.database.as_deref().unwrap_or("-");
        println!(
            "{:<12} {:<30} {:>5} {:<12} {:<12} {}",
            server.name, server.host, server.port, server.user, db, password_status
        );
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
        };
        let result = add.build_from_flags();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("--host is required"));
    }
}
