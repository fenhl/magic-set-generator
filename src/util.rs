use {
    std::{
        convert::Infallible,
        fmt,
        io,
        path::{
            Path,
            PathBuf
        },
        process::{
            Command,
            Output
        }
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

pub trait CommandOutputExt {
    type Ok;

    fn check(&mut self, name: &'static str) -> Result<Self::Ok, Error>;
}

impl CommandOutputExt for Command {
    type Ok = Output;

    fn check(&mut self, name: &'static str) -> Result<Output, Error> {
        let output = self.output().annotate(name)?;
        if output.status.success() {
            Ok(output)
        } else {
            Err(Error::CommandExit(name, output))
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
pub enum Error {
    #[from(ignore)]
    Annotated(String, Box<Error>),
    #[from(ignore)]
    Args(String),
    #[from(ignore)]
    CardGen(String, String),
    #[from(ignore)]
    CardNotFound(String),
    ColorParse(css_color_parser::ColorParseError),
    #[from(ignore)]
    CommandExit(&'static str, Output),
    Db(DbError),
    GitDir(gitdir::host::github::Error),
    InvalidHeaderValue(reqwest::header::InvalidHeaderValue),
    #[from(ignore)]
    Io(io::Error, Option<PathBuf>),
    Json(serde_json::Error),
    LoreSeeker(lore_seeker::Error),
    MissingAsset,
    MissingEnvar(&'static str),
    MissingHomeDir,
    MissingPackage,
    MissingRelease,
    Reqwest(reqwest::Error),
    SameVersion,
    SemVer(semver::SemVerError),
    VersionCommand,
    VersionRegression,
    Zip(zip::result::ZipError)
}

impl From<Infallible> for Error {
    fn from(never: Infallible) -> Error {
        match never {}
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Annotated(msg, e) => write!(f, "{}: {}", msg, e),
            Error::Args(msg) => msg.fmt(f),
            Error::CardGen(card_name, msg) => write!(f, "error generating {}: {}", card_name, msg),
            Error::CardNotFound(card_name) => write!(f, "no card named {:?} found", card_name),
            Error::ColorParse(e) => e.fmt(f),
            Error::CommandExit(cmd, ref output) => write!(f, "subprocess {} exited with status {}", cmd, output.status),
            Error::Db(e) => write!(f, "card database error: {:?}", e), //TODO impl Display for DbError
            Error::GitDir(e) => write!(f, "gitdir error: {:?}", e), //TODO impl Display for gitdir Error
            Error::InvalidHeaderValue(e) => e.fmt(f),
            Error::Io(e, Some(path)) => write!(f, "I/O error at {}: {}", path.display(), e),
            Error::Io(e, None) => write!(f, "I/O error: {}", e),
            Error::Json(e) => e.fmt(f),
            Error::LoreSeeker(e) => write!(f, "Lore Seeker error: {:?}", e), //TODO impl Display for lore_seeker::Error
            Error::MissingAsset => write!(f, "The downlad for your OS is missing from the latest GitHub release."),
            Error::MissingEnvar(var) => write!(f, "missing environment variable: {:?}", var),
            Error::MissingHomeDir => write!(f, "Could not find your user folder."),
            Error::MissingPackage => write!(f, "The binary to be released was not found in Cargo.toml"),
            Error::MissingRelease => write!(f, "The program does not appear to be installed via `cargo install`, but no releases were found on the GitHub repo."),
            Error::Reqwest(e) => if let Some(url) = e.url() {
                write!(f, "error downloading {}: {}", url, e)
            } else {
                write!(f, "reqwest error: {}", e)
            },
            Error::SameVersion => write!(f, "The release being created has the same version as the latest release."),
            Error::SemVer(e) => e.fmt(f),
            Error::VersionCommand => write!(f, "Could not check version of the installed update."),
            Error::VersionRegression => write!(f, "The release being created has a lower version than the latest release."),
            Error::Zip(e) => e.fmt(f)
        }
    }
}

pub trait IntoResultExt {
    type T;

    fn annotate(self, note: impl ToString) -> Result<Self::T, Error>;
}

impl<T, E: Into<Error>> IntoResultExt for Result<T, E> {
    type T = T;

    fn annotate(self, note: impl ToString) -> Result<T, Error> {
        self.map_err(|e| Error::Annotated(note.to_string(), Box::new(e.into())))
    }
}

impl<T> IntoResultExt for io::Result<T> {
    type T = T;

    fn annotate(self, note: impl ToString) -> Result<T, Error> {
        self.map_err(|e| Error::Annotated(note.to_string(), Box::new(e.at_unknown())))
    }
}

pub trait IoResultExt {
    type T;

    fn at(self, path: impl AsRef<Path>) -> Self::T;
    fn at_unknown(self) -> Self::T;
}

impl IoResultExt for io::Error {
    type T = Error;

    fn at(self, path: impl AsRef<Path>) -> Error {
        Error::Io(self, Some(path.as_ref().to_owned()))
    }

    fn at_unknown(self) -> Error {
        Error::Io(self, None)
    }
}

impl<T, E: IoResultExt> IoResultExt for Result<T, E> {
    type T = Result<T, E::T>;

    fn at(self, path: impl AsRef<Path>) -> Result<T, E::T> {
        self.map_err(|e| e.at(path))
    }

    fn at_unknown(self) -> Result<T, E::T> {
        self.map_err(|e| e.at_unknown())
    }
}
