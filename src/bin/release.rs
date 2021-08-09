#![deny(rust_2018_idioms, unused, unused_import_braces, unused_qualifications, warnings)]

use {
    std::{
        cmp::Ordering::*,
        env,
        fs::File,
        io::prelude::*,
        path::Path,
        process::Command,
        time::Duration
    },
    dir_lock::DirLock,
    msegen::{
        github::Repo,
        util::{
            CommandOutputExt as _,
            Error,
            IntoResultExt as _,
            IoResultExt as _
        },
        version::version
    }
};

fn release_client() -> Result<reqwest::blocking::Client, Error> { //TODO return an async client instead
    let mut headers = reqwest::header::HeaderMap::new();
    let mut token = String::default();
    File::open("assets/release-token").at("assets/release-token")?.read_to_string(&mut token).at("assets/release-token")?;
    headers.insert(reqwest::header::AUTHORIZATION, reqwest::header::HeaderValue::from_str(&format!("token {}", token))?);
    headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_static(concat!("magic-set-generator/", env!("CARGO_PKG_VERSION"))));
    Ok(reqwest::blocking::Client::builder().default_headers(headers).timeout(Duration::from_secs(600)).use_rustls_tls().build()?)
}

fn main() -> Result<(), Error> {
    eprintln!("creating reqwest client");
    let client = release_client()?;
    //TODO make sure working dir is clean and on master and up to date with remote and remote is up to date
    let repo = Repo::new("fenhl", "magic-set-generator");
    eprintln!("checking version");
    if let Some(latest_release) = repo.latest_release(&client)? {
        let remote_version = latest_release.version()?;
        match version().cmp(&remote_version) {
            Less => { return Err(Error::VersionRegression); }
            Equal => { return Err(Error::SameVersion); }
            Greater => {}
        }
    }
    eprintln!("waiting for Rust lock");
    let lock_dir = Path::new(&env::var_os("TEMP").ok_or(Error::MissingEnvar("TEMP"))?).join("syncbin-startup-rust.lock");
    let lock = DirLock::new_sync(&lock_dir);
    eprintln!("updating Rust for x86_64");
    Command::new("rustup").arg("update").arg("stable").check("rustup")?;
    eprintln!("updating Rust for i686");
    Command::new("rustup").arg("update").arg("stable-i686-pc-windows-msvc").check("rustup")?;
    drop(lock);
    eprintln!("building msg-win64.exe");
    Command::new("cargo").arg("build").arg("--bin=msg-gui").arg("--release").check("cargo")?;
    eprintln!("building msg-win32.exe");
    Command::new("cargo").arg("+stable-i686-pc-windows-msvc").arg("build").arg("--bin=msg-gui").arg("--release").arg("--target-dir=target-x86").check("cargo")?;
    let release_notes = {
        eprintln!("editing release notes");
        let mut release_notes_file = tempfile::Builder::new()
            .prefix("msg-release-notes")
            .suffix(".md")
            .tempfile().annotate("failed to create tempfile")?;
        Command::new("C:\\Program Files\\Microsoft VS Code\\bin\\code.cmd").arg("--wait").arg(release_notes_file.path()).check("code")?;
        let mut buf = String::default();
        release_notes_file.read_to_string(&mut buf).at(release_notes_file.path())?;
        buf
    };
    eprintln!("creating release");
    let release = repo.create_release(&client, format!("Magic Set Generator {}", version()), format!("v{}", version()), release_notes)?;
    eprintln!("uploading msg-win64.exe");
    repo.release_attach(&client, &release, "msg-win64.exe", "application/vnd.microsoft.portable-executable", File::open("target/release/msg-gui.exe").at("target/release/msg-gui.exe")?)?;
    eprintln!("uploading msg-win32.exe");
    repo.release_attach(&client, &release, "msg-win32.exe", "application/vnd.microsoft.portable-executable", File::open("target-x86/release/msg-gui.exe").at("target-x86/release/msg-gui.exe")?)?;
    eprintln!("publishing release");
    repo.publish_release(&client, release)?;
    Ok(())
}
