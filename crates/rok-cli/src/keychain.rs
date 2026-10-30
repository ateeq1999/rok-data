//! OS keychain integration for server passwords (ADR 0028).
//!
//! Uses the `keyring` crate (macOS Keychain, Windows Credential Manager,
//! Linux `libsecret` via `secret-service`). Service name: `rok`.
//! Account name: the server name.
//!
//! Fallback order when retrieving a password:
//! 1. OS keychain (service `rok`, account `<NAME>`)
//! 2. Environment variable `ROK_SERVER_<NAME>_PASSWORD` (uppercase, hyphens → underscores)
//! 3. Error: `no password for server '<NAME>': not in keychain and ROK_SERVER_<NAME>_PASSWORD not set`
//!
//! Nothing probes the keychain: an operation is attempted, and a keychain
//! that cannot be reached (headless Linux, CI) is reported as `Unavailable`,
//! naming the variable to set instead. `ROK_KEYCHAIN=off` skips the
//! keychain entirely (CI, tests): passwords then come only from the variable.

use std::env;

/// Error type for keychain operations.
#[derive(Debug, thiserror::Error)]
pub(crate) enum KeychainError {
    /// The OS keychain cannot be reached (e.g., headless Linux/CI without libsecret).
    #[error("OS keychain unavailable: {0}")]
    Unavailable(String),

    /// The requested password was not found in the keychain or environment.
    #[error("no password for server '{server}': not in keychain and {env_var} not set")]
    NotFound { server: String, env_var: String },
}

/// The service name used in the keychain.
const SERVICE: &str = "rok";

/// Builds the environment variable name for a server's password fallback.
/// Converts server name to uppercase and replaces hyphens with underscores.
pub(crate) fn env_var_name(server_name: &str) -> String {
    format!(
        "ROK_SERVER_{}_PASSWORD",
        server_name.to_uppercase().replace('-', "_")
    )
}

/// Where passwords are kept: the OS keychain, or a map in tests.
trait KeychainStore {
    /// Stores a password for the given server name.
    fn store(&self, server_name: &str, password: &str) -> Result<(), keyring::Error>;

    /// Retrieves a password for the given server name; `None` when absent.
    fn get(&self, server_name: &str) -> Result<Option<String>, keyring::Error>;

    /// Deletes a password for the given server name; absent is not an error.
    fn delete(&self, server_name: &str) -> Result<(), keyring::Error>;
}

/// The OS keychain, through the `keyring` crate.
struct OsKeychain;

impl KeychainStore for OsKeychain {
    fn store(&self, server_name: &str, password: &str) -> Result<(), keyring::Error> {
        keyring::Entry::new(SERVICE, server_name)?.set_password(password)
    }

    fn get(&self, server_name: &str) -> Result<Option<String>, keyring::Error> {
        match keyring::Entry::new(SERVICE, server_name)?.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn delete(&self, server_name: &str) -> Result<(), keyring::Error> {
        match keyring::Entry::new(SERVICE, server_name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error),
        }
    }
}

/// No keychain at all, for `ROK_KEYCHAIN=off`: nothing is stored or read,
/// so passwords come only from `ROK_SERVER_<NAME>_PASSWORD`.
struct NoKeychain;

impl KeychainStore for NoKeychain {
    fn store(&self, _server_name: &str, _password: &str) -> Result<(), keyring::Error> {
        Err(keyring::Error::NoStorageAccess(
            "the OS keychain is off (ROK_KEYCHAIN=off)".into(),
        ))
    }

    fn get(&self, _server_name: &str) -> Result<Option<String>, keyring::Error> {
        Ok(None)
    }

    fn delete(&self, _server_name: &str) -> Result<(), keyring::Error> {
        Ok(())
    }
}

/// The store `ROK_KEYCHAIN` selects: none when it is `off`, else the OS keychain.
fn os_store() -> &'static dyn KeychainStore {
    let off = env::var("ROK_KEYCHAIN").is_ok_and(|value| value.eq_ignore_ascii_case("off"));
    if off { &NoKeychain } else { &OsKeychain }
}

/// The `Unavailable` error for a keychain failure, naming the fallback variable.
fn unavailable(server_name: &str, error: &keyring::Error) -> KeychainError {
    KeychainError::Unavailable(format!(
        "{error}; set {} instead",
        env_var_name(server_name)
    ))
}

/// Stores a password in the OS keychain.
///
/// Returns `KeychainError::Unavailable`, naming the environment variable to
/// set instead, when the keychain cannot be reached.
pub(crate) fn store_password(server_name: &str, password: &str) -> Result<(), KeychainError> {
    store_in(os_store(), server_name, password)
}

/// Retrieves a password: the OS keychain first, then the environment variable.
///
/// A keychain that cannot be reached falls through to the variable, so CI
/// works with the variable alone. Returns `KeychainError::NotFound` when
/// neither has the password.
pub(crate) fn get_password(server_name: &str) -> Result<String, KeychainError> {
    get_in(os_store(), server_name, |name| env::var(name).ok())
}

/// Deletes a password from the OS keychain; an absent entry is not an error.
///
/// Returns `KeychainError::Unavailable` when the keychain cannot be reached.
pub(crate) fn delete_password(server_name: &str) -> Result<(), KeychainError> {
    delete_in(os_store(), server_name)
}

fn store_in(
    store: &dyn KeychainStore,
    server_name: &str,
    password: &str,
) -> Result<(), KeychainError> {
    store
        .store(server_name, password)
        .map_err(|error| unavailable(server_name, &error))
}

fn get_in(
    store: &dyn KeychainStore,
    server_name: &str,
    env: impl Fn(&str) -> Option<String>,
) -> Result<String, KeychainError> {
    if let Ok(Some(password)) = store.get(server_name) {
        return Ok(password);
    }
    let env_var = env_var_name(server_name);
    env(&env_var).ok_or(KeychainError::NotFound {
        server: server_name.to_owned(),
        env_var,
    })
}

fn delete_in(store: &dyn KeychainStore, server_name: &str) -> Result<(), KeychainError> {
    store
        .delete(server_name)
        .map_err(|error| unavailable(server_name, &error))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::*;

    /// A keychain in memory; `reachable: false` fails every call the way a
    /// headless machine without a secret service does.
    struct MemoryKeychain {
        entries: RefCell<HashMap<String, String>>,
        reachable: bool,
    }

    impl MemoryKeychain {
        fn new() -> Self {
            Self {
                entries: RefCell::new(HashMap::new()),
                reachable: true,
            }
        }

        fn unreachable() -> Self {
            Self {
                reachable: false,
                ..Self::new()
            }
        }

        fn check(&self) -> Result<(), keyring::Error> {
            if self.reachable {
                Ok(())
            } else {
                Err(keyring::Error::PlatformFailure("no secret service".into()))
            }
        }
    }

    impl KeychainStore for MemoryKeychain {
        fn store(&self, server_name: &str, password: &str) -> Result<(), keyring::Error> {
            self.check()?;
            self.entries
                .borrow_mut()
                .insert(server_name.to_owned(), password.to_owned());
            Ok(())
        }

        fn get(&self, server_name: &str) -> Result<Option<String>, keyring::Error> {
            self.check()?;
            Ok(self.entries.borrow().get(server_name).cloned())
        }

        fn delete(&self, server_name: &str) -> Result<(), keyring::Error> {
            self.check()?;
            self.entries.borrow_mut().remove(server_name);
            Ok(())
        }
    }

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn env_var_name_uppercase_and_underscores() {
        assert_eq!(env_var_name("local"), "ROK_SERVER_LOCAL_PASSWORD");
        assert_eq!(env_var_name("staging-db"), "ROK_SERVER_STAGING_DB_PASSWORD");
        assert_eq!(env_var_name("Ci"), "ROK_SERVER_CI_PASSWORD");
    }

    #[test]
    fn a_stored_password_is_read_back_from_the_keychain() {
        let keychain = MemoryKeychain::new();
        store_in(&keychain, "local", "s3cret").expect("store");
        assert_eq!(get_in(&keychain, "local", no_env).expect("get"), "s3cret");
    }

    #[test]
    fn the_keychain_wins_over_the_variable() {
        let keychain = MemoryKeychain::new();
        store_in(&keychain, "local", "from-keychain").expect("store");
        let env = |_: &str| Some("from-env".to_owned());
        assert_eq!(
            get_in(&keychain, "local", env).expect("get"),
            "from-keychain"
        );
    }

    #[test]
    fn an_absent_entry_falls_back_to_the_variable() {
        let keychain = MemoryKeychain::new();
        let env = |name: &str| (name == "ROK_SERVER_LOCAL_PASSWORD").then(|| "from-env".to_owned());
        assert_eq!(get_in(&keychain, "local", env).expect("get"), "from-env");
    }

    #[test]
    fn an_unreachable_keychain_falls_back_to_the_variable() {
        let keychain = MemoryKeychain::unreachable();
        let env = |_: &str| Some("from-env".to_owned());
        assert_eq!(get_in(&keychain, "ci", env).expect("get"), "from-env");
    }

    #[test]
    fn neither_source_is_not_found_naming_the_variable() {
        let keychain = MemoryKeychain::new();
        let error = get_in(&keychain, "staging-db", no_env).expect_err("no password");
        assert!(
            error.to_string().contains("ROK_SERVER_STAGING_DB_PASSWORD"),
            "{error}"
        );
    }

    #[test]
    fn storing_without_a_keychain_names_the_variable_to_set() {
        let keychain = MemoryKeychain::unreachable();
        let error = store_in(&keychain, "ci", "pw").expect_err("unreachable");
        assert!(
            matches!(&error, KeychainError::Unavailable(message) if message.contains("ROK_SERVER_CI_PASSWORD")),
            "{error}"
        );
    }

    #[test]
    fn deleting_is_idempotent() {
        let keychain = MemoryKeychain::new();
        delete_in(&keychain, "gone").expect("absent entry");
        store_in(&keychain, "local", "pw").expect("store");
        delete_in(&keychain, "local").expect("delete");
        delete_in(&keychain, "local").expect("delete again");
        assert!(keychain.entries.borrow().is_empty());
    }

    #[test]
    fn an_error_never_carries_the_password() {
        let keychain = MemoryKeychain::unreachable();
        let error = store_in(&keychain, "local", "hunter2").expect_err("unreachable");
        assert!(!format!("{error} {error:?}").contains("hunter2"));
    }
}
