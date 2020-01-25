//#![deny(rust_2018_idioms, unused, unused_import_braces, unused_qualifications, warnings)]

pub mod args;
pub mod art;
pub mod github;
pub mod mse;
pub mod util;
pub mod version;

use {
    std::{
        collections::BTreeSet,
        fs::File,
        io::{
            self,
            Cursor,
            prelude::*,
            stderr,
            stdout
        }
    },
    gitdir::Host as _,
    mtg::{
        card::{
            Db,
            Layout
        },
        cardtype::CardType
    },
    reqwest::blocking::Client,
    crate::{
        args::{
            ArgsRegular,
            Output
        },
        art::ArtHandler,
        mse::{
            DataFile,
            MseGame
        },
        util::Error
    }
};

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

pub fn client() -> Result<Client, Error> {
    Ok(Client::builder()
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_static(concat!("json-to-mse/", env!("CARGO_PKG_VERSION"))));
            headers
        })
        .build()?
    )
}

pub fn run(client: Client, args: ArgsRegular) -> Result<(), Error> {
    if args.verbose && !args.offline {
        eprint!("[....] checking for updates");
        stderr().flush()?;
        if version::updates_available(&client)? {
            eprintln!("\r[ !! ] an update is available, install with `json-to-mse --update`");
        } else {
            eprintln!("\r[ ok ] json-to-mse is up to date");
        }
    }
    let db = if let Some(ref db_path) = args.database {
        if db_path.is_dir() {
            Db::from_sets_dir(db_path, args.verbose)?
        } else {
            Db::from_mtg_json(serde_json::from_reader(File::open(db_path)?)?, args.verbose)?
        }
    } else if args.offline {
        Db::from_sets_dir(gitdir::GitHub.repo("fenhl/lore-seeker").master()?.join("data").join("sets"), args.verbose)?
    } else {
        Db::download(args.verbose)?
    };
    // normalize card names
    verbose_eprint!(args, "[....] normalizing card names");
    let cards = if args.all_command {
        db.into_iter().collect()
    } else {
        args.cards.iter()
            //TODO also read card names from args.decklists
            //TODO also read card names from queries
            .map(|card_name| db.card(card_name).ok_or_else(|| Error::CardNotFound(card_name.clone())))
            .collect::<Result<BTreeSet<_>, _>>()?
    }.into_iter()
        .flat_map(|card| if let Layout::Meld { top, bottom, .. } = card.layout() {
            vec![top, bottom]
        } else {
            vec![card.primary()]
        })
        .collect::<BTreeSet<_>>();
    verbose_eprintln!(args, "\r[ ok ]");
    if cards.is_empty() {
        verbose_eprintln!(args, "[ !! ] no cards specified, generating empty set file");
    }
    // create set metadata
    let mut set_file = DataFile::new(&args, cards.len());
    let mut schemes_set_file = DataFile::new_schemes(&args, cards.len());
    let mut vanguards_set_file = DataFile::new_vanguards(&args, cards.len());
    //TODO add cards to set
    let mut art_handler = ArtHandler::new(&args, client);
    let mut failed = 0;
    for (i, card) in cards.iter().enumerate() {
        let progress = 4.min(5 * i / cards.len());
        verbose_eprint!(args, "[{}{}] adding cards to set file: {} of {}\r", "=".repeat(progress), ".".repeat(4 - progress), i, cards.len());
        let result = if card.type_line() >= CardType::Scheme {
            if args.include_schemes() {
                set_file.add_card(&card, MseGame::Magic, &args, &mut art_handler)
            } else {
                Ok(())
            }.and_then(|()| schemes_set_file.add_card(&card, MseGame::Archenemy, &args, &mut art_handler))
        } else if card.type_line() >= CardType::Vanguard {
            if args.include_vanguards() {
                set_file.add_card(&card, MseGame::Magic, &args, &mut art_handler)
            } else {
                Ok(())
            }.and_then(|()| vanguards_set_file.add_card(&card, MseGame::Vanguard, &args, &mut art_handler))
        } else {
            set_file.add_card(&card, MseGame::Magic, &args, &mut art_handler)
        };
        match result {
            Ok(()) => {}
            /*
            Err(Error::Uncard) => {
                eprintln!("[ !! ] Failed to add card {}                    ", card_name);
                eprintln!("[ !! ] Un-cards are not supported and will most likely render incorrectly. Re-run with --allow-uncards to generate them anyway.");
            }
            */ //TODO uncomment special case
            Err(e) => {
                if args.verbose {
                    return Err(Error::CardGen(card.to_string(), Box::new(e)));
                } else {
                    eprintln!("[ !! ] Failed to add card {}                    ", card);
                    failed += 1;
                }
            }
        }
    }
    if failed > 0 {
        eprintln!("[ ** ] {} cards failed. Run again with --verbose for a detailed error message", failed);
    }
    verbose_eprintln!(args, "[ ok ] adding cards to set file: {0} of {0}", cards.len());
    //TODO generate stylesheet settings
    //TODO generate footers (or move into constructors)
    // write set zip files
    verbose_eprint!(args, "[....] adding images and saving\r[....]");
    match args.output {
        Output::File(path) => {
            set_file.write_to(File::create(path)?, &mut art_handler)?;
        }
        Output::Stdout => {
            let mut buf = Cursor::<Vec<_>>::default();
            set_file.write_to(&mut buf, &mut art_handler)?;
            verbose_eprint!(args, "\r[=...]");
            io::copy(&mut buf, &mut stdout())?;
        }
    }
    verbose_eprint!(args, "\r[==..]");
    if let Some(schemes_output) = args.schemes_output {
        schemes_output.write_set_file(schemes_set_file, &mut art_handler)?;
    }
    verbose_eprint!(args, "\r[===.]");
    if let Some(vanguards_output) = args.vanguards_output {
        vanguards_output.write_set_file(vanguards_set_file, &mut art_handler)?;
    }
    verbose_eprintln!(args, "\r[ ok ]");
    Ok(())
}
