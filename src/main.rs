#![deny(rust_2018_idioms, unused, unused_import_braces, unused_qualifications, warnings)]

use {
    std::{
        fs::File,
        io::{
            self,
            Cursor,
            prelude::*,
            stderr,
            stdout
        }
    },
    mtg::{
        card::Db,
        cardtype::CardType
    },
    crate::{
        args::{
            Args,
            Output
        },
        mse::{
            DataFile,
            MseGame
        },
        util::Error
    }
};

mod args;
mod github;
mod mse;
mod util;
mod version;

macro_rules! verbose_eprint {
    ($args:expr, $($fmt:tt)+) => {
        if $args.verbose {
            eprint!($($fmt)+);
            stderr().flush()?;
        }
    };
}

macro_rules! verbose_eprintln {
    ($args:expr, $($fmt:tt)+) => {
        if $args.verbose {
            eprintln!($($fmt)+);
        }
    };
}

fn main() -> Result<(), Error> {
    let client = reqwest::Client::builder()
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_static(concat!("json-to-mse/", env!("CARGO_PKG_VERSION"))));
            headers
        })
        .build()?;
    // parse arguments
    let args = match Args::new()? {
        Args::Help => {
            println!("please see https://github.com/fenhl/json-to-mse/blob/{}/README.md for usage instructions", &version::GIT_COMMIT_HASH[..7]);
            return Ok(());
        }
        Args::Update => {
            if version::updates_available(&client)? {
                version::self_update()?;
            } else {
                println!("json-to-mse is up to date.");
            }
            return Ok(());
        }
        Args::Version => {
            println!("JSON to MSE version {} ({})", env!("CARGO_PKG_VERSION"), &version::GIT_COMMIT_HASH[..7]);
            return Ok(());
        }
        Args::Regular(args) => args
    };
    if args.verbose && version::updates_available(&client)? {
        eprintln!("[ !! ] an update is available, install with `json-to-mse --update`");
    }
    // read card names
    let mut card_names = args.cards.clone();
    //TODO also read card names from args.decklists
    //TODO also read card names from queries
    if card_names.is_empty() && !args.all_command {
        verbose_eprintln!(args, "[ !! ] no cards specified, generating empty set file");
    }
    let db = Db::download()?;
    if args.all_command {
        card_names.extend(db.into_iter().map(|card| card.to_string()));
    }
    //TODO normalize card names
    // create set metadata
    let mut set_file = DataFile::new(&args, card_names.len());
    let mut planes_set_file = DataFile::new_planes(&args, card_names.len());
    let mut schemes_set_file = DataFile::new_schemes(&args, card_names.len());
    let mut vanguards_set_file = DataFile::new_vanguards(&args, card_names.len());
    //TODO add cards to set
    let mut failed = 0;
    for (i, card_name) in card_names.iter().enumerate() {
        let progress = 4.min(5 * i / card_names.len());
        verbose_eprint!(args, "[{}{}] adding cards to set file: {} of {}\r", "=".repeat(progress), ".".repeat(4 - progress), i, card_names.len());
        match db.card_fuzzy(card_name).ok_or(Error::CardNotFound).and_then(|card|
            if card.type_line() >= CardType::Plane || card.type_line() >= CardType::Phenomenon {
                if args.include_planes() {
                    set_file.add_card(&card, &db, MseGame::Magic, &args)
                } else {
                    Ok(())
                }.and_then(|()| planes_set_file.add_card(&card, &db, MseGame::Planechase, &args))
            } else if card.type_line() >= CardType::Scheme {
                if args.include_schemes() {
                    set_file.add_card(&card, &db, MseGame::Magic, &args)
                } else {
                    Ok(())
                }.and_then(|()| schemes_set_file.add_card(&card, &db, MseGame::Archenemy, &args))
            } else if card.type_line() >= CardType::Vanguard {
                if args.include_vanguards() {
                    set_file.add_card(&card, &db, MseGame::Magic, &args)
                } else {
                    Ok(())
                }.and_then(|()| vanguards_set_file.add_card(&card, &db, MseGame::Vanguard, &args))
            } else {
                set_file.add_card(&card, &db, MseGame::Magic, &args)
            }
        ) {
            Ok(()) => {}
            /*
            Err(Error::Uncard) => {
                eprintln!("[ !! ] Failed to add card {}        ", card_name);
                eprintln!("[ !! ] Un-cards are not supported and will most likely render incorrectly. Re-run with --allow-uncards to generate them anyway.");
            }
            */ //TODO uncomment special case
            Err(e) => {
                if args.verbose {
                    return Err(Error::CardGen(card_name.into(), Box::new(e)));
                } else {
                    eprintln!("[ !! ] Failed to add card {}        ", card_name);
                    failed += 1;
                }
            }
        }
    }
    if failed > 0 {
        eprintln!("[ ** ] {} cards failed. Run again with --verbose for a detailed error message", failed);
    }
    verbose_eprintln!(args, "[ ok ] adding cards to set file: {0} of {0}", card_names.len());
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
