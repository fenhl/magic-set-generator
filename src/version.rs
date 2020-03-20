//! Contains versioning information and self-update functionality.

use {
    std::{
        env::current_exe,
        fmt,
        process::{
            Command,
            Stdio
        }
    },
    dirs::home_dir,
    lazy_static::lazy_static,
    regex::Regex,
    reqwest::blocking::Client,
    semver::Version,
    smart_default::SmartDefault,
    crate::{
        github::Repo,
        util::*
    }
};
#[cfg(windows)] use {
    std::fs::{
        self,
        File
    },
    itertools::Itertools as _
};

include!(concat!(env!("OUT_DIR"), "/version.rs")); // defines GIT_COMMIT_HASH

#[cfg(all(windows, target_arch = "x86"))]
const PLATFORM_SUFFIX: &str = "win32.exe";
#[cfg(all(windows, target_arch = "x86_64"))]
const PLATFORM_SUFFIX: &str = "win64.exe";

lazy_static! {
    static ref VERSION_REGEX: Regex = Regex::new("^JSON to MSE version ([^ ]) \\([0-9a-z]{7}\\)$").expect("could not parse version regex");
}

#[derive(SmartDefault)]
pub enum UpdateProgress {
    #[default]
    NotStarted,
    Running, //TODO more detail
    RestartToUpdate(Version),
    NoUpdateAvailable,
    Error(Error)
}

impl fmt::Display for UpdateProgress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UpdateProgress::NotStarted => write!(f, "Update check not started."),
            UpdateProgress::Running => write!(f, "checking for updates…"),
            UpdateProgress::RestartToUpdate(new) => write!(f, "MSG {} is available (you have version {}). Relaunch to update.", new, version()),
            UpdateProgress::NoUpdateAvailable => write!(f, "MSG is up to date (version {}).", version()),
            UpdateProgress::Error(e) => write!(f, "An error occurred during self-update: {}", e)
        }
    }
}

/// Returns `Ok(None)` if MSG is up to date, or updates to the latest version then returns the new version.
pub fn self_update(client: &Client) -> Result<Option<Version>, Error> {
    if !updates_available(&client)? {
        return Ok(None);
    }
    let current_exe = current_exe().at_unknown()?;
    let cargo_bin = home_dir().ok_or(Error::MissingHomeDir)?.join(".cargo").join("bin");
    #[cfg(windows)] let cargo_installed_path = cargo_bin.join("msg.exe");
    #[cfg(windows)] let tmp_path = cargo_bin.join("msg.exe.old");
    #[cfg(windows)] { if tmp_path.exists() { fs::remove_file(&tmp_path).at(&tmp_path)?; } }
    #[cfg(not(windows))] let cargo_installed_path = cargo_bin.join("msg");
    #[cfg(windows)] let cargo_gui_installed_path = cargo_bin.join("msg-gui.exe");
    #[cfg(windows)] let tmp_gui_path = cargo_bin.join("msg-gui.exe.old");
    #[cfg(windows)] { if tmp_gui_path.exists() { fs::remove_file(&tmp_gui_path).at(&tmp_gui_path)?; } }
    #[cfg(not(windows))] let cargo_gui_installed_path = cargo_bin.join("msg-gui");
    if current_exe == cargo_installed_path {
        // always update to the latest commit if installed via cargo
        #[cfg(windows)] fs::rename(&cargo_installed_path, &tmp_path).at(tmp_path)?;
        Command::new("cargo")
            .arg("install-update")
            .arg("--git")
            .arg("msegen")
            //.create_no_window() // also suppresses output in PowerShell //TODO redirect output instead?
            .check("cargo")?;
        let ver_out = Command::new(cargo_installed_path)
            .arg("--version")
            .stdout(Stdio::piped())
            .check("msg")?
            .stdout;
        Ok(Some(VERSION_REGEX.captures(&String::from_utf8_lossy(&ver_out)).ok_or(Error::VersionCommand)?[1].parse()?)) //TODO return None if commit hashes match
    } else if current_exe == cargo_gui_installed_path {
        // always update to the latest commit if installed via cargo
        #[cfg(windows)] fs::rename(cargo_gui_installed_path, &tmp_gui_path).at(tmp_gui_path)?;
        Command::new("cargo")
            .arg("install-update")
            .arg("--git")
            .arg("msegen")
            .create_no_window() //TODO redirect output?
            .check("cargo")?;
        let ver_out = Command::new(cargo_installed_path)
            .arg("--version")
            .stdout(Stdio::piped())
            .check("msg")?
            .stdout;
        Ok(Some(VERSION_REGEX.captures(&String::from_utf8_lossy(&ver_out)).ok_or(Error::VersionCommand)?[1].parse()?)) //TODO return None if commit hashes match
    } else {
        // update to the latest release
        #[cfg(windows)] {
            fs::rename(&current_exe, tempfile::Builder::new().prefix("magic-set-generator").suffix(".old").tempfile().at_unknown()?).at(&current_exe)?;
            let repo = Repo::new("fenhl", "magic-set-generator");
            if let Some(release) = repo.latest_release(client)? {
                let new_ver = release.version()?;
                let (asset,) = release.assets.into_iter()
                    .filter(|asset| asset.name.ends_with(PLATFORM_SUFFIX))
                    .collect_tuple().ok_or(Error::MissingAsset)?;
                let mut response = client.get(asset.browser_download_url).send()?.error_for_status()?;
                response.copy_to(&mut File::open(&current_exe).at(current_exe)?)?;
                return Ok(Some(new_ver));
            }
        }
        Err(Error::MissingRelease) //TODO fall back to latest commit
    }
}

/// Returns `Ok(false)` if MSG is up to date, or `Ok(true)` if an update is available.
pub fn updates_available(client: &Client) -> Result<bool, Error> {
    let cargo_bin = home_dir().ok_or(Error::MissingHomeDir)?.join(".cargo").join("bin");
    #[cfg(windows)] let cargo_installed_path = cargo_bin.join("msg.exe");
    #[cfg(not(windows))] let cargo_installed_path = cargo_bin.join("msg");
    #[cfg(windows)] let cargo_gui_installed_path = cargo_bin.join("msg-gui.exe");
    #[cfg(not(windows))] let cargo_gui_installed_path = cargo_bin.join("msg-gui");
    let repo = Repo::new("fenhl", "magic-set-generator");
    if current_exe().at_unknown()? == cargo_installed_path || current_exe().at_unknown()? == cargo_gui_installed_path {
        // always update to the latest commit if installed via cargo
        let branch = repo.branch(client, "master")?;
        Ok(GIT_COMMIT_HASH != branch.commit.sha)
    } else {
        // update to the latest release, or the latest commit if no releases exist
        if let Some(release) = repo.latest_release(client)? {
            let new_ver = release.version()?;
            Ok(new_ver > version())
        } else {
            let branch = repo.branch(client, "master")?;
            Ok(GIT_COMMIT_HASH != branch.commit.sha)
        }
    }
}

/// The version of the msegen crate.
pub fn version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("failed to parse current version")
}
