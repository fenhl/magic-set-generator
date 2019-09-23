#![deny(rust_2018_idioms, unused, unused_import_braces, unused_qualifications, warnings)]

use {
    std::io::{
        self,
        Cursor,
        prelude::*,
        stderr,
        stdout
    },
    derive_more::From,
    mtg::card::{
        Db,
        DbError
    },
    crate::{
        args::Args,
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
    Args,
    Db(DbError),
    Io(io::Error)
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
    //TODO also read card names/commands from stdin, unless it's a tty
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
    if let Some(output) = args.output {
        set_file.write_to(output)?;
    } else {
        let mut buf = Cursor::<Vec<_>>::default();
        set_file.write_to(&mut buf)?;
        verbose_eprint!(args, "\r[=...]");
        io::copy(&mut buf, &mut stdout())?;
    }
    verbose_eprint!(args, "\r[==..]");
    if let Some(planes_output) = args.planes_output {
        planes_set_file.write_to(planes_output)?;
    }
    verbose_eprint!(args, "\r[===.]");
    if let Some(schemes_output) = args.schemes_output {
        schemes_set_file.write_to(schemes_output)?;
    }
    verbose_eprint!(args, "\r[====]");
    if let Some(vanguards_output) = args.vanguards_output {
        vanguards_set_file.write_to(vanguards_output)?;
    }
    verbose_eprintln!(args, "\r[ ok ]");
    Ok(())
}
