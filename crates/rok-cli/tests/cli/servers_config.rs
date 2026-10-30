//! Unit tests for `servers.toml` parsing, config directory resolution,
//! and read/write round-trips.

use std::fs;
use tempfile::TempDir;

use rok_data_cli::config::{
    ServerConfig, ServersConfig, config_dir, read_servers, servers_path, write_servers,
};

/// A minimal valid server entry.
fn minimal_server() -> ServerConfig {
    ServerConfig {
        name: "local".to_owned(),
        host: "localhost".to_owned(),
        port: 5432,
        user: "postgres".to_owned(),
        database: Some("postgres".to_owned()),
        sslmode: Some("prefer".to_owned()),
    }
}

#[test]
fn config_dir_returns_platform_appropriate_path() {
    let dir = config_dir();
    // On all platforms, the path should end with "rok"
    assert!(dir.to_string_lossy().ends_with("rok"));
    // And it should be an absolute path
    assert!(dir.is_absolute());
}

#[test]
fn servers_path_returns_config_dir_joined_servers_toml() {
    let path = servers_path();
    let dir = config_dir();
    assert_eq!(path, dir.join("servers.toml"));
}

#[test]
fn server_config_round_trip_toml() {
    let server = minimal_server();
    let toml = toml::to_string(&server).expect("serialize");
    let parsed: ServerConfig = toml::from_str(&toml).expect("deserialize");
    assert_eq!(server, parsed);
}

#[test]
fn servers_config_round_trip_toml() {
    let config = ServersConfig {
        servers: vec![minimal_server()],
    };
    let toml = toml::to_string(&config).expect("serialize");
    let parsed: ServersConfig = toml::from_str(&toml).expect("deserialize");
    assert_eq!(config, parsed);
}

#[test]
fn servers_config_multiple_servers() {
    let mut server1 = minimal_server();
    server1.name = "local".to_owned();

    let mut server2 = minimal_server();
    server2.name = "staging".to_owned();
    server2.host = "staging-db.example.com".to_owned();
    server2.user = "rok".to_owned();
    server2.sslmode = Some("require".to_owned());

    let config = ServersConfig {
        servers: vec![server1, server2],
    };
    let toml = toml::to_string(&config).expect("serialize");
    let parsed: ServersConfig = toml::from_str(&toml).expect("deserialize");
    assert_eq!(config, parsed);
}

#[test]
fn unknown_key_in_server_config_is_error() {
    let toml = r#"
[[servers]]
name = "local"
host = "localhost"
port = 5432
user = "postgres"
unknown_field = "should fail"
"#;
    let result: Result<ServersConfig, _> = toml::from_str(toml);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("unknown field"), "error was: {err}");
}

#[test]
fn unknown_key_at_top_level_is_error() {
    let toml = r#"
unknown_top_level = "should fail"
[[servers]]
name = "local"
host = "localhost"
port = 5432
user = "postgres"
"#;
    let result: Result<ServersConfig, _> = toml::from_str(toml);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("unknown field"), "error was: {err}");
}

#[test]
fn missing_file_returns_empty_config() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("servers.toml");
    // File doesn't exist
    let config = read_servers(Some(&path)).expect("read missing file");
    assert_eq!(config.servers.len(), 0);
}

#[test]
fn write_then_read_round_trip() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("servers.toml");

    let original = ServersConfig {
        servers: vec![minimal_server()],
    };

    write_servers(&path, &original).expect("write");
    let read_back = read_servers(Some(&path)).expect("read");

    assert_eq!(original, read_back);
}

#[test]
fn write_creates_file_with_0600_permissions() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("servers.toml");

    let config = ServersConfig {
        servers: vec![minimal_server()],
    };

    write_servers(&path, &config).expect("write");

    let metadata = fs::metadata(&path).expect("metadata");
    let _permissions = metadata.permissions();
    // On Unix, check that the file is not readable by others
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = _permissions.mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {:o}", mode);
    }
    // On Windows, just verify the file was created and is readable by us
    #[cfg(windows)]
    {
        assert!(metadata.len() > 0);
    }
}

#[test]
fn write_uses_atomic_rename() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("servers.toml");

    let config = ServersConfig {
        servers: vec![minimal_server()],
    };

    // Write the file
    write_servers(&path, &config).expect("write");

    // The file should exist and be valid
    assert!(path.exists());
    let read_back = read_servers(Some(&path)).expect("read");
    assert_eq!(config, read_back);

    // Verify no temp files left behind
    let entries: Vec<_> = fs::read_dir(&dir).expect("read dir").collect();
    let temp_files: Vec<_> = entries
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(".servers.toml"))
        .collect();
    assert!(
        temp_files.is_empty(),
        "found temp files: {:?}",
        temp_files.iter().map(|e| e.file_name()).collect::<Vec<_>>()
    );
}

#[test]
fn server_config_optional_fields_can_be_omitted() {
    let toml = r#"
[[servers]]
name = "minimal"
host = "localhost"
port = 5432
user = "postgres"
"#;
    let config: ServersConfig = toml::from_str(toml).expect("parse");
    assert_eq!(config.servers.len(), 1);
    let server = &config.servers[0];
    assert_eq!(server.name, "minimal");
    assert_eq!(server.database, None);
    assert_eq!(server.sslmode, None);
}

#[test]
fn server_config_sslmode_values() {
    let valid_modes = [
        "disable",
        "allow",
        "prefer",
        "require",
        "verify-ca",
        "verify-full",
    ];
    for mode in valid_modes {
        let toml = format!(
            r#"
[[servers]]
name = "test"
host = "localhost"
port = 5432
user = "postgres"
sslmode = "{}"
"#,
            mode
        );
        let config: ServersConfig =
            toml::from_str(&toml).unwrap_or_else(|_| panic!("parse {}", mode));
        assert_eq!(config.servers[0].sslmode, Some(mode.to_owned()));
    }
}

#[test]
fn server_config_invalid_sslmode_is_accepted_at_parse_time() {
    // serde doesn't validate enum values by default; validation happens at use time
    let toml = r#"
[[servers]]
name = "test"
host = "localhost"
port = 5432
user = "postgres"
sslmode = "invalid"
"#;
    let config: ServersConfig = toml::from_str(toml).expect("parse");
    assert_eq!(config.servers[0].sslmode, Some("invalid".to_owned()));
}
