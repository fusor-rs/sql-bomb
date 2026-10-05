use semver::Version;
use serde::Deserialize;
use std::{env, ffi::OsStr, fs, path::Path, process::Command};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const RELEASE_TIMEOUT_SECONDS: &str = "30";
// The release workflow uploads this after binaries and crates are published.
const READY_ASSET: &str = "install.sh";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
}

pub(crate) fn run() -> Result<()> {
    let current = Version::parse(env!("CARGO_PKG_VERSION"))?;
    let Some(version) = latest(&current)? else {
        println!("{} {current} is already up to date.", crate::cli::COMMAND);
        return Ok(());
    };
    let executable = fs::canonicalize(env::current_exe()?)?;
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    println!("Upgrading {} {current} → {version}", crate::cli::COMMAND);
    let status = match cargo_installation(&executable, &cargo)? {
        Some(root) => Command::new(&cargo)
            .args(["install", env!("CARGO_PKG_NAME"), "--locked", "--version"])
            .arg(format!("={version}"))
            .arg("--root")
            .arg(root)
            .status()?,
        None => installer()
            .arg(version.to_string())
            .env("SQL_BOMB_BIN", &executable)
            .status()?,
    };
    if !status.success() {
        return Err(
            format!("Upgrade failed ({status}); see the messages above and try again").into(),
        );
    }
    Ok(())
}

fn latest(current: &Version) -> Result<Option<Version>> {
    let release = release()?;
    let version = Version::parse(release.tag_name.trim_start_matches('v'))
        .map_err(|error| format!("Invalid release version; check the release tag: {error}"))?;
    if release.draft || release.prerelease || !version.pre.is_empty() {
        return Err("The latest release is not stable; try again after it is published".into());
    }
    if !version.cmp_precedence(current).is_gt() {
        return Ok(None);
    }
    let output = installer()
        .arg("--archive")
        .arg(version.to_string())
        .output()?;
    if !output.status.success() {
        return Err(String::from_utf8(output.stderr)?.into());
    }
    let archive = String::from_utf8(output.stdout)?;
    for required in [
        READY_ASSET,
        archive.trim(),
        &format!("{}.sha256", archive.trim()),
    ] {
        if !release.assets.iter().any(|asset| asset.name == required) {
            return Err(
                format!("Release {version} is still being published; try again later").into(),
            );
        }
    }
    Ok(Some(version))
}

fn release() -> Result<Release> {
    let repository = env!("CARGO_PKG_REPOSITORY")
        .strip_prefix("https://github.com/")
        .expect("the package repository is hosted on GitHub");
    let url = env::var_os("SQL_BOMB_RELEASE_URL").unwrap_or_else(|| {
        format!("https://api.github.com/repos/{repository}/releases/latest").into()
    });
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--max-time",
            RELEASE_TIMEOUT_SECONDS,
        ])
        .arg(url)
        .output()
        .map_err(|error| format!("Cannot check releases; install curl and try again: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Cannot check releases; try again later: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("Invalid release response; try again later: {error}").into())
}

fn installer() -> Command {
    let mut command = Command::new("sh");
    command.args(["-c", include_str!("../install.sh"), "sql-bomb-installer"]);
    command
}

fn cargo_installation<'path>(
    executable: &'path Path,
    cargo: &OsStr,
) -> Result<Option<&'path Path>> {
    let Some(directory) = executable.parent().filter(|path| path.ends_with("bin")) else {
        return Ok(None);
    };
    let root = directory
        .parent()
        .ok_or("The installation has no parent directory")?;
    if !root.join(".crates.toml").try_exists()? {
        return Ok(None);
    }
    let output = Command::new(cargo)
        .args(["install", "--list", "--root"])
        .arg(root)
        .output()
        .map_err(|error| format!("Cannot inspect the installation; install Cargo: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Cannot inspect the Cargo installation; check Cargo and try again: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    let package = format!("{} v", env!("CARGO_PKG_NAME"));
    let listing = String::from_utf8(output.stdout)?;
    let mut lines = listing.lines();
    if !lines.any(|line| line.starts_with(&package)) {
        return Ok(None);
    }
    Ok(lines
        .take_while(|line| line.starts_with(' '))
        .any(|line| Some(OsStr::new(line.trim())) == executable.file_name())
        .then_some(root))
}
