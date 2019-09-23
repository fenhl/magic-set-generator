#![deny(rust_2018_idioms, unused, unused_import_braces, unused_qualifications, warnings)]

use {
    std::{
        convert::Infallible,
        fs::File,
        io::{
            self,
            Cursor,
            prelude::*,
            stderr,
            stdout
        }
    },
    derive_more::From,
    mtg::card::{
        Db,
        DbError
    },
    crate::{
        args::{
            Args,
            Output
        },
        mse::DataFile
    }
};

mod args;
mod mse;
mod version;

macro_rules! verbose_eprint {
    ($args:expr, $fmt:tt) => {
        if $args.verbose {
            eprint!($fmt);
            stderr().flush()?;
        }
    };
}

macro_rules! verbose_eprintln {
    ($args:expr, $fmt:tt) => {
        if $args.verbose {
            eprintln!($fmt);
        }
    };
}

#[derive(Debug, From)]
pub(crate) enum Error {
    Args(String),
    Db(DbError),
    Io(io::Error)
}

impl From<Infallible> for Error {
    fn from(never: Infallible) -> Error {
        match never {}
    }
}

fn main() -> Result<(), Error> {
    // pargs arguments
    let args = match Args::new()? {
        Args::Help => {
            println!("please see https://github.com/fenhl/json-to-mse/blob/{}/README.md for usage instructions", &version::GIT_COMMIT_HASH[..7]);
            return Ok(());
        }
        Args::Version => {
            println!("JSON to MSE version {} ({})", env!("CARGO_PKG_VERSION"), &version::GIT_COMMIT_HASH[..7]);
            return Ok(());
        }
        Args::Regular(args) => args
    };
    // read card names
    let /*mut*/ card_names = args.cards.clone();
    //TODO also read card names from args.decklists
    //TODO also read card names from queries
    if card_names.is_empty() && !args.all_command {
        verbose_eprintln!(args, "[ !! ] no cards specified, generating empty set file");
    }
    let _ /*db*/ = Db::download()?;
    if args.all_command {
        //card_names = db.into_iter().map(|card| card.to_string()).collect(); //TODO uncomment
    }
    //TODO normalize card names
    // create set metadata
    let set_file = DataFile::new(&args, card_names.len());
    let planes_set_file = DataFile::new_planes(&args, card_names.len());
    let schemes_set_file = DataFile::new_schemes(&args, card_names.len());
    let vanguards_set_file = DataFile::new_vanguards(&args, card_names.len());
    //TODO add cards to set
    //TODO generate stylesheet settings
    //TODO generate footers (or move into constructors)
    // write set zip files
    verbose_eprint!(args, "[....] adding images and saving\r[....]");
    match args.output {
        Output::File(path) => {
            set_file.write_to(File::create(path)?)?;
        }
        Output::Stdout => {
            let mut buf = Cursor::<Vec<_>>::default();
            set_file.write_to(&mut buf)?;
            verbose_eprint!(args, "\r[=...]");
            io::copy(&mut buf, &mut stdout())?;
        }
    }
    verbose_eprint!(args, "\r[==..]");
    if let Some(planes_output) = args.planes_output {
        planes_output.write_set_file(planes_set_file)?;
    }
    verbose_eprint!(args, "\r[===.]");
    if let Some(schemes_output) = args.schemes_output {
        schemes_output.write_set_file(schemes_set_file)?;
    }
    verbose_eprint!(args, "\r[====]");
    if let Some(vanguards_output) = args.vanguards_output {
        vanguards_output.write_set_file(vanguards_set_file)?;
    }
    verbose_eprintln!(args, "\r[ ok ]");
    Ok(())
}
