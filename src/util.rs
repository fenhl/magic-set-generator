use {
    std::{
        convert::Infallible,
        io,
        process::Command
    },
    derive_more::From,
    mtg::card::DbError
};
#[cfg(windows)] use std::os::windows::process::CommandExt as _;

pub(crate) trait CommandExt {
    fn create_no_window(&mut self) -> &mut Command;
}

impl CommandExt for Command {
    #[cfg(windows)] fn create_no_window(&mut self) -> &mut Command {
        self.creation_flags(0x0800_0000)
    }

    #[cfg(not(windows))] fn create_no_window(&mut self) -> &mut Command {
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

pub(crate) trait StrExt {
    fn to_uppercase_first(&self) -> String;
}

impl StrExt for str {
    fn to_uppercase_first(&self) -> String {
        let mut chars = self.chars();
        if let Some(first) = chars.next() {
            format!("{}{}", first.to_uppercase(), chars.collect::<String>())
        } else {
            String::default()
        }
    }
}

#[derive(Debug, From)]
pub(crate) enum Error {
    Args(String),
    CardGen(String, Box<Error>),
    CardNotFound(String),
    ColorParse(css_color_parser::ColorParseError),
    CommandExit(&'static str),
    Db(DbError),
    GitDir(gitdir::host::github::Error),
    Io(io::Error),
    Json(serde_json::Error),
    MissingHomeDir,
    Reqwest(reqwest::Error),
    SelfUpdateUnimplemented,
    TagNotFound,
    //Uncard,
    Zip(zip::result::ZipError)
}

impl From<Infallible> for Error {
    fn from(never: Infallible) -> Error {
        match never {}
    }
}
