//! Contains versioning information and self-update functionality.

use {
    std::env::current_exe,
    dirs::home_dir,
    itertools::Itertools as _,
    crate::{
        Error,
        github::Repo
    }
};

include!(concat!(env!("OUT_DIR"), "/version.rs"));

pub(crate) fn self_update() -> Result<(), Error> {
    if current_exe()? == home_dir().ok_or(Error::MissingHomeDir)?.join(".cargo/bin/json-to-mse") {
        return Err(Error::SelfUpdateUnimplemented); //TODO use `cargo install-update`?
    } else {
        return Err(Error::SelfUpdateUnimplemented);
    }
}

/// Returns `Ok(false)` if `json-to-mse` is up to date, or `Ok(true)` if an update is available.
pub(crate) fn updates_available(client: &reqwest::Client) -> Result<bool, Error> {
    let repo = Repo::new("fenhl", "json-to-mse");
    let tag_name = repo.latest_release(&client)?.tag_name;
    let current_hash = if let Some((tag,)) = repo.tags(&client)?.into_iter().filter(|tag| tag.name == tag_name).collect_tuple() {
        tag.commit.sha
    } else {
        return Err(Error::TagNotFound);
    };
    Ok(GIT_COMMIT_HASH != current_hash)
}
