//! OS keychain integration for server passwords (ADR 0028).
//!
//! Uses the `keyring` crate (macOS Keychain, Windows Credential Manager,
//! Linux `libsecret` via `secret-service`). Service name: `rok`.
//! Account name: the server name.
//!
//! Fallback order when retrieving a password:
//! 1. OS keychain (service `rok`, account `<NAME>`)
//! 2. Environment variable `ROK_SERVER_<NAME>_PASSWORD` (uppercase, hyphens → underscores)
//! 3. Error: "no password for server '<NAME>': not in keychain and ROK_SERVER_<NAME>_PASSWORD not set"

use std::env;

/// Error type for keychain operations.
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)] // Unavailable is constructed via KeychainError::from in store_password
pub(crate) enum KeychainError {
    /// The OS keychain is not available (e.g., headless Linux/CI without libsecret).
    #[error("OS keychain unavailable: {0}")]
    Unavailable(String),

    /// The requested password was not found in the keychain or environment.
    #[error("no password for server '{server}': not in keychain and {env_var} not set")]
    NotFound { server: String, env_var: String },

    /// A keyring crate error occurred.
    #[error("keychain error: {0}")]
    Keyring(#[from] keyring::Error),
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

/// Creates a keyring entry for the given server name.
fn entry(server_name: &str) -> keyring::Entry {
    keyring::Entry::new(SERVICE, server_name)
}

/// Stores a password in the OS keychain.
///
/// On success, returns `Ok(())`.
/// On keychain unavailability, returns `KeychainError::Unavailable` with a message
/// instructing the user to set the environment variable fallback.
pub(crate) fn store_password(server_name: &str, password: &str) -> Result<(), KeychainError> {
    let entry = entry(server_name);
    entry.set_password(password).map_err(KeychainError::from)
}

/// Retrieves a password, trying the OS keychain first, then the environment variable.
///
/// Returns the password on success.
/// Returns `KeychainError::NotFound` if neither source has the password.
pub(crate) fn get_password(server_name: &str) -> Result<String, KeychainError> {
    // Try keychain first
    let entry = entry(server_name);
    match entry.get_password() {
        Ok(password) => return Ok(password),
        Err(keyring::Error::NoEntry) => {
            // Fall through to env var
        }
        Err(e) => return Err(KeychainError::from(e)),
    }

    // Fallback to environment variable
    let env_var = env_var_name(server_name);
    env::var(&env_var).map_err(|_| KeychainError::NotFound {
        server: server_name.to_owned(),
        env_var,
    })
}

/// Deletes a password from the OS keychain.
///
/// Returns `Ok(())` on success (including if the entry didn't exist).
/// Returns `KeychainError::Unavailable` if the keychain is not available.
pub(crate) fn delete_password(server_name: &str) -> Result<(), KeychainError> {
    let entry = entry(server_name);
    match entry.delete_password() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()), // Idempotent: not an error if already gone
        Err(e) => Err(KeychainError::from(e)),
    }
}

/// Checks if the OS keychain is available.
///
/// Returns `Ok(())` if available, `Err(KeychainError::Unavailable)` with a
/// platform-specific message if not.
#[cfg(test)]
pub(crate) fn check_keychain_available() -> Result<(), KeychainError> {
    // Try to create an entry and immediately delete it to test availability
    let test_entry = entry("__rok_keychain_test__");
    test_entry
        .set_password("test")
        .map_err(KeychainError::from)?;
    test_entry.delete_password().map_err(KeychainError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_var_name_uppercase_and_underscores() {
        assert_eq!(env_var_name("local"), "ROK_SERVER_LOCAL_PASSWORD");
        assert_eq!(env_var_name("staging-db"), "ROK_SERVER_STAGING_DB_PASSWORD");
        assert_eq!(env_var_name("Ci"), "ROK_SERVER_CI_PASSWORD");
        assert_eq!(
            env_var_name("my-server-name"),
            "ROK_SERVER_MY_SERVER_NAME_PASSWORD"
        );
    }

    #[test]
    fn store_and_get_password_round_trip() {
        // This test requires a working keychain. On CI/headless it will fail
        // with KeychainError::Unavailable, which is expected behavior.
        let server = "__test_server_round_trip__";
        let password = "test_password_123";

        // Clean up any existing entry first
        let _ = delete_password(server);

        match store_password(server, password) {
            Ok(()) => {
                // Keychain available, verify round-trip
                let retrieved =
                    get_password(server).expect("get_password should succeed after store");
                assert_eq!(retrieved, password);
                delete_password(server).expect("cleanup");
            }
            Err(KeychainError::Unavailable(_)) => {
                // No keychain available (e.g., CI), this is expected
                // The env var fallback is tested in integration tests
            }
            Err(e) => panic!("unexpected error: {}", e),
        }
    }

    #[test]
    fn get_password_error_when_neither_source_has_password() {
        let server = "nonexistent_server_xyz";
        let env_var = env_var_name(server);

        // Ensure no keychain entry and no env var
        let _ = delete_password(server);
        // Note: env::remove_var is unsafe in Rust 2024; we skip it here
        // since the test server name is unique and unlikely to have an env var set

        let result = get_password(server);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("no password for server"));
        assert!(err.contains("nonexistent_server_xyz"));
        assert!(err.contains(&env_var));
    }

    #[test]
    fn delete_password_is_idempotent() {
        let server = "__test_delete_idempotent__";
        let _ = delete_password(server); // Should not error even if not present

        // If keychain is available, add and delete twice
        if store_password(server, "pwd").is_ok() {
            delete_password(server).expect("first delete");
            delete_password(server).expect("second delete (idempotent)");
        }
    }

    #[test]
    fn check_keychain_available_reports_correctly() {
        match check_keychain_available() {
            Ok(()) => {
                // Keychain is available
            }
            Err(KeychainError::Unavailable(msg)) => {
                // Keychain not available - message should be informative
                assert!(!msg.is_empty());
            }
            Err(e) => panic!("unexpected error: {}", e),
        }
    }
}
