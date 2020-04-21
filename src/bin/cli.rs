#![deny(rust_2018_idioms, unused, unused_import_braces, unused_qualifications, warnings)]

use {
    std::io::{
        prelude::*,
        stderr
    },
    async_std::task,
    gres::Task as _,
    msegen::{
        Run,
        args::Args,
        util::{
            Error,
            IoResultExt as _
        },
        version
    }
};

macro_rules! verbose_eprint {
    ($args:expr, $($fmt:tt)+) => {
        if $args.verbose {
            eprint!($($fmt)+);
            stderr().flush().at_unknown()?;
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
    let client = msegen::client()?;
    // parse arguments
    let args = match Args::new()? {
        Args::Help => {
            println!("please see https://github.com/fenhl/magic-set-generator#readme for usage instructions");
            return Ok(());
        }
        Args::Update => {
            if let Some(new_ver) = version::self_update(&client)? {
                println!("Magic Set Generator has been updated to version {}.", new_ver);
            } else {
                println!("Magic Set Generator is up to date.");
            }
            return Ok(());
        }
        Args::Version => {
            println!("Magic Set Generator version {} ({})", env!("CARGO_PKG_VERSION"), &version::GIT_COMMIT_HASH[..7]);
            return Ok(());
        }
        Args::Regular(args) => args
    };
    let mut run = msegen::Run::new(client, args.clone());
    loop {
        match task::block_on(run.run()) {
            Ok(Ok(())) => {
                verbose_eprintln!(args, "\r[ ok ]");
                break;
            }
            Ok(Err(e)) => { return Err(e); }
            Err(r) => {
                run = r;
                match run {
                    Run::CheckForUpdates { .. } => {
                        eprint!("[....] checking for updates");
                        stderr().flush().at_unknown()?;
                    }
                    Run::LoadDb { updates_available: Some(true), .. } => { eprintln!("\r[ !! ] an update is available, install with `msegen --update`"); }
                    Run::LoadDb { updates_available: Some(false), .. } => { eprintln!("\r[ ok ] Magic Set Generator is up to date"); }
                    Run::NormalizeCardNames { .. } => { verbose_eprint!(args, "[....] normalizing card names"); }
                    Run::CreateSetMetadata { ref cards, .. } => {
                        verbose_eprintln!(args, "\r[ ok ]");
                        if cards.is_empty() {
                            verbose_eprintln!(args, "[ !! ] no cards specified, generating empty set file");
                        }
                    }
                    Run::AddNextCard { added_cards, ref cards, ref error, .. } => {
                        if let Some((card_name, debug, display)) = error {
                            /*
                            Err(Error::Uncard) => {
                                eprintln!("[ !! ] Failed to add card {}                    ", card_name);
                                eprintln!("[ !! ] Un-cards are not supported and will most likely render incorrectly. Re-run with --allow-uncards to generate them anyway.");
                            }
                            */ //TODO uncomment special case
                            if args.verbose {
                                eprintln!("[ !! ] Failed to add card {}: {}", card_name, display);
                                return Err(Error::CardGen(card_name.to_string(), debug.to_string()));
                            } else {
                                eprintln!("[ !! ] Failed to add card {}                    ", card_name);
                            }
                        }
                        let total_cards = added_cards + cards.len();
                        let progress = 4.min(5 * added_cards / total_cards);
                        verbose_eprint!(args, "[{}{}] adding cards to set file: {} of {}\r", "=".repeat(progress), ".".repeat(4 - progress), added_cards, total_cards);
                    }
                    Run::GenerateStylesheetSettings { failed, .. } => {
                        if failed > 0 {
                            eprintln!("[ ** ] {} cards failed. Run again with --verbose for a detailed error message", failed);
                        }
                    }
                    Run::WriteMain { .. } => { verbose_eprint!(args, "[....] adding images and saving\r[....]"); }
                    Run::CopyMain { .. } => { verbose_eprint!(args, "\r[=...]"); }
                    Run::WriteSchemes { .. } => { verbose_eprint!(args, "\r[==..]"); }
                    Run::WriteVanguards { .. } => { verbose_eprint!(args, "\r[===.]"); }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}
