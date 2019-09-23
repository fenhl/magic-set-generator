use std::{
    env,
    fs::File,
    io::{
        self,
        prelude::*
    },
    path::Path,
    process::Command
};

/// Modified from <https://stackoverflow.com/questions/43753491/include-git-commit-hash-as-string-into-rust-program>
fn get_git_hash() -> String {
    let git_dir = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap())
        .join(".git");
    let commit = Command::new("git")
        .arg(format!("--git-dir={}", git_dir.display()))
        .arg("rev-parse")
        .arg("--verify")
        .arg("HEAD")
        .current_dir(env::var("CARGO_MANIFEST_DIR").unwrap())
        .output();
    match commit {
        Ok(commit_output) => {
            if !commit_output.status.success() {
                panic!("git exited with {}, stderr: {:?}", commit_output.status, String::from_utf8_lossy(&commit_output.stderr));
            }
            let commit_string = String::from_utf8_lossy(&commit_output.stdout);
            commit_string.lines().next().expect(&format!("Incorrect formatting of git commit: {:?}", commit_string)).to_owned()
        }
        Err(e) => { panic!("Cannot get git commit: {}", e); }
    }
}

fn main() -> Result<(), io::Error> {
    println!("cargo:rerun-if-changed=nonexistent.foo"); // check a nonexistent file to make sure build script is always run (see https://github.com/rust-lang/cargo/issues/4213 and https://github.com/rust-lang/cargo/issues/5663)
    let mut f = File::create(Path::new(&env::var("OUT_DIR").unwrap()).join("version.rs"))?;
    writeln!(f, "/// The hash of the current commit of the json-to-mse repo at compile time.")?;
    writeln!(f, "pub(crate) const GIT_COMMIT_HASH: &str = \"{}\";", get_git_hash())?;
    Ok(())
}
