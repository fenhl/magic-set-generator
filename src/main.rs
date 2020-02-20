#![deny(rust_2018_idioms, unused, unused_import_braces, unused_qualifications, warnings)]

use msegen::{
    args::Args,
    util::Error,
    version
};

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
    msegen::run(client, args)?;
    Ok(())
}
