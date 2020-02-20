#![deny(rust_2018_idioms, unused, unused_import_braces, unused_qualifications, warnings)]

use {
    std::{
        env,
        fs::File,
        io::prelude::*,
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

fn main() {
    println!("cargo:rerun-if-changed=nonexistent.foo"); // check a nonexistent file to make sure build script is always run (see https://github.com/rust-lang/cargo/issues/4213 and https://github.com/rust-lang/cargo/issues/5663)
    let mut f = File::create(Path::new(&env::var("OUT_DIR").unwrap()).join("version.rs")).unwrap();
    writeln!(f, "/// The hash of the current commit of the magic-set-generator repo at compile time.").unwrap();
    writeln!(f, "pub const GIT_COMMIT_HASH: &str = \"{}\";", match get_git_hash() {
        Ok(hash) => hash,
        Err(e) => { panic!("Cannot get git commit: {}\n{:?}", e, e); }
    }).unwrap();
}
