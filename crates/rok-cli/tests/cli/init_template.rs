//! `rok sqlc init --template <NAME>` copies a ready-made project's files
//! from the rok-templates crate, embedded in the binary, each only when absent. It
//! leaves out rok-data-sqlc's generated ones (`rok sqlc generate` writes them), but
//! copies the template's pinned `Cargo.lock`, and generates nothing.

use super::*;

/// The template's files as `init` copies them: every one but
/// rok-data-sqlc's generated ones and a build's `target/`, with `\n` line endings, and
/// `<path>.template` (its `Cargo.toml`) as `<path>`.
fn copied_files(template: &str) -> Result<BTreeMap<PathBuf, String>> {
    Ok(common::embedded_template(template)
        .into_iter()
        .filter(|(path, text)| {
            !path.starts_with("target")
                && !common::is_generated(text)
                // The schema artifact (engine ADR 0010) carries no marker for
                // JSON: `rok sqlc generate` writes it, `init` leaves it out.
                && path != Path::new("src/db/schema.json")
        })
        .map(|(path, text)| {
            let name = path.to_string_lossy().into_owned();
            let path = name.strip_suffix(".template").map_or(path, PathBuf::from);
            (path, text.replace("\r\n", "\n"))
        })
        .collect())
}

#[test]
fn init_template_copies_the_templates_files_but_the_generated_ones() -> Result<()> {
    let dir = TempDir::new()?;
    let project = dir.path().join("app");
    let output = run_binary(&["init", "-t", "actix", project.to_str().unwrap_or_default()]);
    assert_success(&output);

    let expected = copied_files("actix-api")?;
    assert!(
        expected.contains_key(Path::new("rok-data-sqlc.toml"))
            && expected.contains_key(Path::new("Cargo.lock")),
        "{expected:?}"
    );
    assert!(
        common::embedded_template("actix-api")
            .values()
            .any(|text| common::is_generated(text)),
        "the template holds generated files to leave out"
    );
    assert_eq!(read_tree(&project)?, expected);

    let stdout = String::from_utf8_lossy(&output.stdout);
    for file in expected.keys() {
        assert!(
            stdout.contains(&format!("scaffolded {}", shown(&project.join(file)))),
            "{} not reported:\n{stdout}",
            file.display()
        );
    }
    for step in ["rok sqlc generate", "cargo run"] {
        assert!(
            stdout.contains(step),
            "next step `{step}` missing:\n{stdout}"
        );
    }
    Ok(())
}

/// The axum template is copied the same way: its files but the generated
/// ones, and its pinned `Cargo.lock`.
#[test]
fn init_template_copies_the_axum_templates_files_but_the_generated_ones() -> Result<()> {
    let dir = TempDir::new()?;
    let project = dir.path().join("app");
    let output = run_binary(&["init", "-t", "axum", project.to_str().unwrap_or_default()]);
    assert_success(&output);

    let expected = copied_files("axum-api")?;
    assert!(
        expected.contains_key(Path::new("rok-data-sqlc.toml"))
            && expected.contains_key(Path::new("Cargo.lock")),
        "{expected:?}"
    );
    assert_eq!(read_tree(&project)?, expected);
    Ok(())
}

/// The grpc template is copied the same way: its files but the generated
/// ones (the `.proto` files and the tonic services, which `rok sqlc
/// generate` writes), its `build.rs` and its pinned `Cargo.lock`.
#[test]
fn init_template_copies_the_grpc_templates_files_but_the_generated_ones() -> Result<()> {
    let dir = TempDir::new()?;
    let project = dir.path().join("app");
    let output = run_binary(&["init", "-t", "grpc", project.to_str().unwrap_or_default()]);
    assert_success(&output);

    let expected = copied_files("grpc-api")?;
    for file in ["rok-data-sqlc.toml", "Cargo.lock", "build.rs"] {
        assert!(
            expected.contains_key(Path::new(file)),
            "{file}: {expected:?}"
        );
    }
    for generated in ["proto/users.proto", "src/features/users/grpc.rs"] {
        assert!(
            !expected.contains_key(Path::new(generated)),
            "{generated} is copied"
        );
    }
    assert_eq!(read_tree(&project)?, expected);
    Ok(())
}

#[test]
fn init_template_keeps_existing_files() -> Result<()> {
    let dir = TempDir::new()?;
    fs::write(
        dir.path().join("rok-data-sqlc.toml"),
        "schema = \"schema.sql\"\n",
    )?;
    let output = run_binary_in(dir.path(), &["init", "--template", "actix"]);
    assert_success(&output);
    assert_eq!(
        fs::read_to_string(dir.path().join("rok-data-sqlc.toml"))?,
        "schema = \"schema.sql\"\n"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("scaffolded rok-data-sqlc.toml"),
        "{stdout}"
    );
    assert!(stdout.contains("scaffolded migrations"), "{stdout}");
    Ok(())
}

#[test]
fn init_template_rejects_an_unknown_name_listing_the_valid_ones() -> Result<()> {
    let dir = TempDir::new()?;
    let output = run_binary_in(dir.path(), &["init", "-t", "rocket"]);
    assert!(!output.status.success(), "an unknown template must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        ["rocket", "actix", "axum", "grpc", "monorepo"]
            .iter()
            .all(|name| stderr.contains(name)),
        "{stderr}"
    );
    assert!(read_tree(dir.path())?.is_empty(), "nothing written");
    Ok(())
}

/// The step lines after `Next steps:`.
fn next_steps(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .skip_while(|line| *line != "Next steps:")
        .skip(1)
        .collect()
}

#[test]
fn init_template_prints_numbered_next_steps_without_the_dependencies_it_copied() -> Result<()> {
    let dir = TempDir::new()?;
    let output = run_binary_in(dir.path(), &["init", "-t", "actix"]);
    assert_success(&output);
    assert!(dir.path().join("Cargo.toml").is_file());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let steps = next_steps(&stdout);
    assert_eq!(
        steps,
        [
            "  1. run `rok sqlc generate` to generate src/db/, src/shared/ and src/features/ from schema/",
            "  2. set DATABASE_URL, JWT_SECRET and TOTP_ENCRYPTION_KEY (see .env.example) and apply migrations/",
            "  3. `cargo test`, then `cargo run`",
        ],
        "{stdout}"
    );
    Ok(())
}

#[test]
fn init_template_into_a_package_asks_for_the_templates_dependencies() -> Result<()> {
    let dir = TempDir::new()?;
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"shop\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    let output = run_binary_in(dir.path(), &["init", "-t", "actix"]);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let steps = next_steps(&stdout);
    assert_eq!(
        steps.first().copied(),
        Some(
            "  1. add the template's dependencies and lints (its README.md lists them) to Cargo.toml"
        ),
        "{stdout}"
    );
    assert_eq!(steps.len(), 4, "{stdout}");
    Ok(())
}

/// A Monorepo scaffold names every App after the project and `--org`, and
/// its next steps run the api App by its renamed package.
#[test]
fn init_monorepo_names_the_apps_after_the_project_and_org() -> Result<()> {
    let dir = TempDir::new()?;
    let output = run_binary_in(
        dir.path(),
        &["init", "my-shop", "-t", "monorepo", "--org", "com.acme"],
    );
    assert_success(&output);
    let project = dir.path().join("my-shop");
    let gradle = fs::read_to_string(project.join("apps/mobile/android/app/build.gradle.kts"))?;
    assert!(
        gradle.contains("applicationId = \"com.acme.my_shop\""),
        "{gradle}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let steps = next_steps(&stdout);
    assert!(
        steps.contains(&"  3. `cargo test` in apps/api, then `cargo run -p my-shop-api`"),
        "{stdout}"
    );
    // Only `rok gen` regenerates the Apps and the Clients: the engine's door
    // names it too, not its own one-crate `rok sqlc generate`.
    assert!(
        steps
            .first()
            .is_some_and(|step| step
                .starts_with("  1. run `rok gen` from the root to generate every named App")),
        "{stdout}"
    );
    assert!(!stdout.contains("rok sqlc generate"), "{stdout}");
    Ok(())
}

/// `--org` takes a reverse-domain name; anything else is a usage error
/// that writes nothing.
#[test]
fn init_rejects_an_org_that_is_not_a_reverse_domain_name() -> Result<()> {
    let dir = TempDir::new()?;
    let output = run_binary_in(dir.path(), &["init", "-t", "monorepo", "--org", "Acme Inc"]);
    assert!(!output.status.success(), "a malformed org must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("is not a reverse-domain name"), "{stderr}");
    assert!(read_tree(dir.path())?.is_empty(), "nothing written");
    Ok(())
}

/// `--org` names a template's Apps: without `--template` it is a usage
/// error.
#[test]
fn init_org_requires_a_template() -> Result<()> {
    let dir = TempDir::new()?;
    let output = run_binary_in(dir.path(), &["init", "--org", "com.acme"]);
    assert!(
        !output.status.success(),
        "an org without a template must fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--template"), "{stderr}");
    assert!(read_tree(dir.path())?.is_empty(), "nothing written");
    Ok(())
}
