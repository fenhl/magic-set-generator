use {
    std::{
        convert::Infallible,
        io,
        process::Command
    },
    derive_more::From,
    mtg::card::DbError
};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt as _;

pub(crate) trait CommandExt {
    fn create_no_window(&mut self) -> &mut Command;
}

impl CommandExt for Command {
    #[cfg(target_os = "windows")]
    fn create_no_window(&mut self) -> &mut Command {
        self.creation_flags(0x0800_0000)
    }

    #[cfg(not(target_os = "windows"))]
    fn create_no_window(&mut self) -> &mut Command {
        self
    }
}

pub(crate) trait CommandOutputExt {
    type Ok;

    fn check(&mut self, name: &'static str) -> Result<Self::Ok, Error>;
}

impl CommandOutputExt for Command {
    type Ok = ();

    fn check(&mut self, name: &'static str) -> Result<(), Error> {
        if self.status()?.success() {
            Ok(())
        } else {
            Err(Error::CommandExit(name))
        }
    }
}

#[derive(Debug, From)]
pub(crate) enum Error {
    Args(String),
    CardGen(String, Box<Error>),
    CardNotFound,
    CommandExit(&'static str),
    Db(DbError),
    Io(io::Error),
    MissingHomeDir,
    Reqwest(reqwest::Error),
    SelfUpdateUnimplemented,
    TagNotFound,
    //Uncard
}

impl From<Infallible> for Error {
    fn from(never: Infallible) -> Error {
        match never {}
    }
}
