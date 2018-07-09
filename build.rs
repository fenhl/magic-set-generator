use std::{
    fs::File,
    io::{
        self,
        prelude::*
    },
    process::Command
};

/// Modified from <https://stackoverflow.com/questions/43753491/include-git-commit-hash-as-string-into-rust-program>
fn get_git_hash() -> String {
    let commit = Command::new("git")
        .arg("rev-parse")
        .arg("--verify")
        .arg("HEAD")
        .output();
    match commit {
        Ok(commit_output) => {
            let commit_string = String::from_utf8_lossy(&commit_output.stdout);
            commit_string.lines().next().expect("Incorrect formatting of git commit").to_owned()
        }
        Err(e) => { panic!("Can not get git commit: {}", e); }
    }
}

fn main() -> Result<(), io::Error> {
    let mut f = File::create("src/version.rs")?;
    writeln!(f, "const GIT_COMMIT_HASH: &str = \"{}\";", get_git_hash())?;
    Ok(())
}
