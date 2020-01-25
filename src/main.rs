#![deny(rust_2018_idioms, unused, unused_import_braces, unused_qualifications, warnings)]

use json_to_mse::{
    args::Args,
    util::Error,
    version
};

fn main() -> Result<(), Error> {
    let client = json_to_mse::client()?;
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
    json_to_mse::run(client, args)?;
    Ok(())
}
