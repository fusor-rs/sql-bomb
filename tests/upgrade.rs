use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::Command,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const FUTURE_VERSION: &str = "999.0.0";
const INSTALLER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/install.sh");
const EXECUTABLE: &str = "boom";
const INSTALLATION: &str = "installed sql-bomb";

struct ReleaseArchive {
    release: Value,
    checksum: PathBuf,
}

#[test]
fn upgrades_preserve_installations_until_a_release_is_verified() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let directory = directory.path();
    let bin_directory = directory.join(INSTALLATION).join("bin");
    let installed = bin_directory.join(EXECUTABLE);
    fs::create_dir_all(&bin_directory)?;
    fs::copy(env!("CARGO_BIN_EXE_boom"), &installed)?;
    let link = directory.join(EXECUTABLE);
    symlink(&installed, &link)?;
    assert_eq!(
        success(boom(&link, directory).arg("--version"))?,
        format!("{EXECUTABLE} {CURRENT_VERSION}\n").as_bytes()
    );
    rejected(
        boom(&link, directory).args(["upgrade", "unexpected"]),
        "Unexpected argument",
    )?;

    keeps_current_releases(&link, directory)?;
    let release = archive(directory, FUTURE_VERSION)?;
    rejects_unusable_releases(&link, directory, &release.release)?;
    rejects_bad_downloads(&link, directory, &release)?;
    uses_cargo_for_tracked_installations(&link, directory, &release.release)?;
    assert_eq!(fs::read(&installed)?, fs::read(env!("CARGO_BIN_EXE_boom"))?);
    assert_eq!(fs::read_link(&link)?, installed);
    assert_eq!(fs::read_dir(bin_directory)?.count(), 1);

    archive(directory, CURRENT_VERSION)?;
    fs::copy("/usr/bin/true", &installed)?;
    success(
        Command::new("sh")
            .arg(INSTALLER)
            .arg(CURRENT_VERSION)
            .env("SQL_BOMB_BIN", &installed)
            .env("SQL_BOMB_DOWNLOAD_BASE", download_base(directory)),
    )?;
    assert_eq!(
        success(boom(&link, directory).arg("--version"))?,
        format!("{EXECUTABLE} {CURRENT_VERSION}\n").as_bytes()
    );
    assert_eq!(fs::read_link(link)?, installed);
    Ok(())
}

fn boom(executable: &Path, directory: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .env(
            "SQL_BOMB_RELEASE_URL",
            format!("file://{}/release.json", directory.display()),
        )
        .env("SQL_BOMB_DOWNLOAD_BASE", download_base(directory))
        .env("SQL_BOMB_INSTALL", directory.join("other installation"))
        .env("CARGO", env!("CARGO"))
        .env("CARGO_NET_OFFLINE", "true");
    command
}

fn download_base(directory: &Path) -> String {
    format!("file://{}/downloads", directory.display())
}

fn success(command: &mut Command) -> Result<Vec<u8>> {
    let output = command.output()?;
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

fn keeps_current_releases(executable: &Path, directory: &Path) -> Result<()> {
    for version in [
        CURRENT_VERSION,
        "0.0.0",
        &format!("{CURRENT_VERSION}+build"),
    ] {
        let release = json!({
            "tag_name": format!("v{version}"),
            "draft": false,
            "prerelease": false,
            "assets": [],
        });
        fs::write(directory.join("release.json"), release.to_string())?;
        assert_eq!(
            success(boom(executable, directory).arg("upgrade"))?,
            format!("{EXECUTABLE} {CURRENT_VERSION} is already up to date.\n").as_bytes()
        );
    }
    Ok(())
}

fn rejects_unusable_releases(executable: &Path, directory: &Path, release: &Value) -> Result<()> {
    for (field, value, message) in [
        ("assets", json!([]), "still being published"),
        ("draft", json!(true), "not stable"),
        ("prerelease", json!(true), "not stable"),
        (
            "tag_name",
            json!(format!("v{FUTURE_VERSION}-rc.1")),
            "not stable",
        ),
        ("tag_name", json!("invalid"), "unexpected character"),
    ] {
        let mut release = release.clone();
        release[field] = value;
        fs::write(directory.join("release.json"), release.to_string())?;
        rejected(boom(executable, directory).arg("upgrade"), message)?;
    }
    fs::write(directory.join("release.json"), "invalid JSON")?;
    rejected(
        boom(executable, directory).arg("upgrade"),
        "Invalid release response",
    )?;
    fs::remove_file(directory.join("release.json"))?;
    rejected(
        boom(executable, directory).arg("upgrade"),
        "Cannot check releases",
    )?;
    Ok(())
}

fn rejects_bad_downloads(
    executable: &Path,
    directory: &Path,
    archive: &ReleaseArchive,
) -> Result<()> {
    fs::write(directory.join("release.json"), archive.release.to_string())?;
    rejected(
        boom(executable, directory).arg("upgrade"),
        "downloaded boom has the wrong version",
    )?;
    fs::write(&archive.checksum, "invalid\n")?;
    rejected(
        boom(executable, directory).arg("upgrade"),
        "checksum mismatch",
    )?;
    fs::remove_file(&archive.checksum)?;
    rejected(
        boom(executable, directory).arg("upgrade"),
        "cannot download",
    )?;
    Ok(())
}

fn uses_cargo_for_tracked_installations(
    executable: &Path,
    directory: &Path,
    release: &Value,
) -> Result<()> {
    fs::write(directory.join("release.json"), release.to_string())?;
    fs::write(
        directory.join(INSTALLATION).join(".crates.toml"),
        format!(
            r#"[v1]
"sql-bomb {CURRENT_VERSION} (registry+https://github.com/rust-lang/crates.io-index)" = [
    "{EXECUTABLE}",
]
"#
        ),
    )?;
    rejected(
        boom(executable, directory)
            .arg("upgrade")
            .env("CARGO_HOME", directory.join("empty cargo home")),
        "Upgrade failed (exit status: 101)",
    )?;
    Ok(())
}

fn rejected(command: &mut Command, message: &str) -> Result<()> {
    let output = command.output()?;
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains(message), "{error}");
    Ok(())
}

fn archive(directory: &Path, version: &str) -> Result<ReleaseArchive> {
    let archive = success(
        Command::new("sh")
            .arg(INSTALLER)
            .args(["--archive", version]),
    )?;
    let archive = String::from_utf8(archive)?;
    let archive = archive.trim();
    let member = archive
        .strip_suffix(".tar.gz")
        .expect("release archives use tar.gz");
    let downloads = directory.join("downloads").join(format!("v{version}"));
    fs::create_dir_all(downloads.join(member))?;
    fs::copy(
        env!("CARGO_BIN_EXE_boom"),
        downloads.join(member).join(EXECUTABLE),
    )?;
    success(
        Command::new("tar")
            .current_dir(&downloads)
            .args(["-czf", archive, member]),
    )?;
    let digest = success(
        Command::new("shasum")
            .current_dir(&downloads)
            .args(["-a", "256", archive]),
    )?;
    let checksum = downloads.join(format!("{archive}.sha256"));
    fs::write(&checksum, digest)?;
    let release = json!({
        "tag_name": format!("v{version}"),
        "draft": false,
        "prerelease": false,
        "assets": [
            { "name": "install.sh" },
            { "name": archive },
            { "name": format!("{archive}.sha256") },
        ],
    });
    Ok(ReleaseArchive { release, checksum })
}
