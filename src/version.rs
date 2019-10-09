//! Contains versioning information and self-update functionality.

use {
    std::{
        env::current_exe,
        process::Command
    },
    dirs::home_dir,
    itertools::Itertools as _,
    crate::{
        github::Repo,
        util::*
    }
};

include!(concat!(env!("OUT_DIR"), "/version.rs"));

pub(crate) fn self_update() -> Result<(), Error> {
    if current_exe()? == home_dir().ok_or(Error::MissingHomeDir)?.join(".cargo/bin/json-to-mse") {
        Command::new("cargo")
            .arg("install-update")
            .arg("--git")
            .arg("json-to-mse")
            .create_no_window()
            .check("cargo")
    } else {
        //TODO update from GitHub releases
        return Err(Error::SelfUpdateUnimplemented);
    }
}

/// Returns `Ok(false)` if `json-to-mse` is up to date, or `Ok(true)` if an update is available.
pub(crate) fn updates_available(client: &reqwest::Client) -> Result<bool, Error> {
    let repo = Repo::new("fenhl", "json-to-mse");
    if let Some(release) = repo.latest_release(client)? {
        let current_hash = if let Some((tag,)) = repo.tags(&client)?.into_iter().filter(|tag| tag.name == release.tag_name).collect_tuple() {
            tag.commit.sha
        } else {
            return Err(Error::TagNotFound);
        };
        Ok(GIT_COMMIT_HASH != current_hash)
    } else {
        let branch = repo.branch(client, "riir")?;
        Ok(GIT_COMMIT_HASH != branch.commit.sha)
    }
}
