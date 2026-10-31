//! The `apps/mobile` App of the monorepo template (workspace ADR 0027),
//! made the way a user makes it: `rok init --template monorepo` into a
//! scratch directory writes it whole — its platform folders' binary files
//! byte for byte, its `.env.example`, and Firebase's placeholders, never a
//! real project's configuration.

use anyhow::{Result, ensure};

use crate::common;
use crate::rok_data_sqlc;

/// The scaffold writes the mobile App's binary files byte for byte, its
/// `.env.example`, and the Firebase placeholders.
#[test]
fn monorepo_scaffold_writes_the_mobile_app_whole() -> Result<()> {
    let project = tempfile::TempDir::new()?;
    rok_data_sqlc(project.path(), &["init", "--template", "monorepo"])?;
    let template = common::template_dir("monorepo").join("apps/mobile");
    let mobile = project.path().join("apps/mobile");
    for file in [
        "android/app/src/main/res/mipmap-hdpi/ic_launcher.png",
        "ios/Runner/Assets.xcassets/AppIcon.appiconset/Icon-App-1024x1024@1x.png",
        "web/favicon.png",
    ] {
        ensure!(
            std::fs::read(mobile.join(file))? == std::fs::read(template.join(file))?,
            "the scaffolded apps/mobile/{file} differs from the template's"
        );
    }
    for file in [
        ".env.example",
        "pubspec.yaml",
        "pubspec.lock",
        "analysis_options.yaml",
    ] {
        ensure!(
            mobile.join(file).is_file(),
            "the scaffold has no apps/mobile/{file}"
        );
    }
    for file in [
        "android/app/google-services.json",
        "ios/Runner/GoogleService-Info.plist",
    ] {
        let text = std::fs::read_to_string(mobile.join(file))?;
        ensure!(
            text.contains("your-firebase-project"),
            "apps/mobile/{file} is not the Firebase placeholder"
        );
    }
    Ok(())
}
