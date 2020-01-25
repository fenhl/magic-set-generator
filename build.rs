#![deny(rust_2018_idioms, unused, unused_import_braces, unused_qualifications, warnings)]

use {
    std::{
        env,
        fs::File,
        io::{
            self,
            prelude::*
        },
        path::Path
    },
    git2::{
        Oid,
        Repository
    }
};

/// Modified from <https://stackoverflow.com/questions/43753491/include-git-commit-hash-as-string-into-rust-program>
fn get_git_hash() -> Result<Oid, git2::Error> {
    Ok(
        Repository::open(env::var_os("CARGO_MANIFEST_DIR").unwrap())?
            .revparse_single("HEAD")?
            .id()
    )
}

fn main() -> Result<(), io::Error> {
    println!("cargo:rerun-if-changed=nonexistent.foo"); // check a nonexistent file to make sure build script is always run (see https://github.com/rust-lang/cargo/issues/4213 and https://github.com/rust-lang/cargo/issues/5663)
    let mut f = File::create(Path::new(&env::var("OUT_DIR").unwrap()).join("version.rs"))?;
    writeln!(f, "/// The hash of the current commit of the json-to-mse repo at compile time.")?;
    match get_git_hash() {
        Ok(hash) => { writeln!(f, "pub const GIT_COMMIT_HASH: &str = \"{}\";", hash)?; }
        Err(e) => { panic!("Cannot get git commit: {}\n{:?}", e, e); }
    }
    Ok(())
}
