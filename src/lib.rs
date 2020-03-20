#![deny(rust_2018_idioms, unused, unused_import_braces, unused_qualifications, warnings)]

pub mod args;
pub mod art;
pub mod github;
pub mod mse;
pub mod util;
pub mod version;

use {
    std::{
        collections::BTreeSet,
        fmt,
        fs::File,
        io::{
            self,
            Cursor,
            stdout
        }
    },
    async_trait::async_trait,
    gitdir::Host as _,
    gres::{
        Percent,
        Progress,
        Task
    },
    lazy_static::lazy_static,
    mtg::{
        card::{
            Card,
            Db,
            Layout
        },
        cardtype::CardType
    },
    regex::Regex,
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
        util::{
            Error,
            IoResultExt as _
        }
    }
};

macro_rules! task_try {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(e) => { return Ok(Err(e.into())); }
        }
    };
}

lazy_static! {
    static ref SPLIT_CARD_REGEX: Regex = Regex::new("^(.+?) ?/+ ?.+$").expect("failed to build split card regex");
}

#[derive(Debug, Clone)]
pub enum Run {
    NotStarted {
        client: Client,
        args: ArgsRegular
    },
    CheckForUpdates {
        client: Client,
        args: ArgsRegular
    },
    LoadDb {
        client: Client,
        args: ArgsRegular,
        updates_available: Option<bool>
    },
    NormalizeCardNames {
        client: Client,
        args: ArgsRegular,
        db: Db
    },
    CreateSetMetadata {
        client: Client,
        args: ArgsRegular,
        cards: BTreeSet<Card>
    },
    AddNextCard {
        client: Client,
        args: ArgsRegular,
        cards: Vec<Card>,
        added_cards: usize,
        failed: usize,
        error: Option<(String, String, String)>,
        art_handler: ArtHandler,
        set_file: DataFile,
        schemes_set_file: DataFile,
        vanguards_set_file: DataFile
    },
    GenerateStylesheetSettings {
        args: ArgsRegular,
        failed: usize,
        art_handler: ArtHandler,
        set_file: DataFile,
        schemes_set_file: DataFile,
        vanguards_set_file: DataFile
    },
    GenerateFooters {
        args: ArgsRegular,
        art_handler: ArtHandler,
        set_file: DataFile,
        schemes_set_file: DataFile,
        vanguards_set_file: DataFile
    },
    WriteMain {
        args: ArgsRegular,
        art_handler: ArtHandler,
        set_file: DataFile,
        schemes_set_file: DataFile,
        vanguards_set_file: DataFile
    },
    CopyMain {
        args: ArgsRegular,
        buf: Cursor<Vec<u8>>,
        art_handler: ArtHandler,
        schemes_set_file: DataFile,
        vanguards_set_file: DataFile
    },
    WriteSchemes {
        args: ArgsRegular,
        art_handler: ArtHandler,
        schemes_set_file: DataFile,
        vanguards_set_file: DataFile
    },
    WriteVanguards {
        vanguards_output: Option<Output>,
        art_handler: ArtHandler,
        vanguards_set_file: DataFile
    }
}

impl Run {
    pub fn new(client: Client, args: ArgsRegular) -> Run {
        Run::NotStarted { client, args }
    }
}

impl Progress for Run {
    fn progress(&self) -> Percent {
        match self { //TODO balance out
            Run::NotStarted { .. } => Percent::default(),
            Run::CheckForUpdates { .. } => Percent::new(1),
            Run::LoadDb { .. } => Percent::new(2),
            Run::NormalizeCardNames { .. } => Percent::new(3),
            Run::CreateSetMetadata { .. } => Percent::new(4),
            Run::AddNextCard { added_cards, cards, .. } => {
                let total_cards = added_cards + cards.len();
                let progress = 88.min(89 * added_cards / total_cards) as u8;
                Percent::new(5 + progress)
            }
            Run::GenerateStylesheetSettings { .. } => Percent::new(94),
            Run::GenerateFooters { .. } => Percent::new(95),
            Run::WriteMain { .. } => Percent::new(96),
            Run::CopyMain { .. } => Percent::new(97),
            Run::WriteSchemes { .. } => Percent::new(98),
            Run::WriteVanguards { .. } => Percent::new(99)
        }
    }
}

impl fmt::Display for Run {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Run::NotStarted { .. } => write!(f, "not started"),
            Run::CheckForUpdates { .. } => write!(f, "checking for updates"),
            Run::LoadDb { .. } => write!(f, "loading card database"),
            Run::NormalizeCardNames { .. } => write!(f, "normalizing card names"),
            Run::CreateSetMetadata { .. } => write!(f, "generating set metadata"),
            Run::AddNextCard { added_cards, ref cards, failed, .. } => if failed == 0 {
                write!(f, "adding cards: {}/{}", added_cards, added_cards + cards.len())
            } else {
                write!(f, "adding cards: {}/{} ({} failed)", added_cards, added_cards + cards.len(), failed)
            },
            Run::GenerateStylesheetSettings { .. } => write!(f, "generating stylesheet settings"),
            Run::GenerateFooters { .. } => write!(f, "generating set footers"),
            Run::WriteMain { .. } => write!(f, "adding images and converting to MSE format"),
            Run::CopyMain { .. } => write!(f, "saving"),
            Run::WriteSchemes { .. } => write!(f, "saving schemes"),
            Run::WriteVanguards { .. } => write!(f, "saving vanguards")
        }
    }
}

#[async_trait]
impl Task<Result<(), Error>> for Run {
    async fn run(self) -> Result<Result<(), Error>, Run> {
        match self {
            Run::NotStarted { client, args } => if args.verbose && !args.offline {
                Err(Run::CheckForUpdates { client, args })
            } else {
                Err(Run::LoadDb {
                    updates_available: None,
                    client, args
                })
            },
            Run::CheckForUpdates { client, args } => Err(Run::LoadDb {
                updates_available: Some(task_try!(version::updates_available(&client))),
                client, args
            }),
            Run::LoadDb { client, args, .. } => Err(Run::NormalizeCardNames {
                db: if let Some(ref db_path) = args.database {
                    if db_path.is_dir() {
                        task_try!(Db::from_sets_dir(db_path, args.verbose))
                    } else {
                        task_try!(Db::from_mtg_json(task_try!(serde_json::from_reader(task_try!(File::open(db_path).at(db_path)))), args.verbose))
                    }
                } else if args.offline {
                    task_try!(Db::from_sets_dir(task_try!(gitdir::GitHub.repo("fenhl/lore-seeker").master()).join("data").join("sets"), args.verbose))
                } else {
                    task_try!(Db::download(args.verbose))
                },
                client, args
            }),
            Run::NormalizeCardNames { client, args, db } => Err(Run::CreateSetMetadata {
                cards: if args.all_command {
                    db.into_iter().collect()
                } else {
                    task_try!(args.cards.iter()
                        //TODO also read card names from args.decklists
                        //TODO also read card names from queries
                        .map(|card_name| card_name.replace('’', "'"))
                        .map(|card_name| match SPLIT_CARD_REGEX.captures(&card_name) {
                            Some(captures) => captures[1].to_owned(),
                            None => card_name.to_owned()
                        })
                        .map(|card_name| db.card(&card_name).ok_or_else(|| Error::CardNotFound(card_name)))
                        .collect::<Result<BTreeSet<_>, _>>()
                    )
                }.into_iter()
                    .flat_map(|card| if let Layout::Meld { top, bottom, .. } = card.layout() {
                        vec![top, bottom]
                    } else {
                        vec![card.primary()]
                    })
                    .collect(),
                client, args
            }),
            Run::CreateSetMetadata { client, args, cards } => Err(Run::AddNextCard {
                added_cards: 0,
                failed: 0,
                error: None,
                art_handler: ArtHandler::new(&args, client.clone()),
                set_file: DataFile::new(&args, cards.len()),
                schemes_set_file: DataFile::new_schemes(&args, cards.len()),
                vanguards_set_file: DataFile::new_vanguards(&args, cards.len()),
                client, args,
                cards: cards.into_iter().collect()
            }),
            Run::AddNextCard { client, args, mut cards, added_cards, failed, mut art_handler, mut set_file, mut schemes_set_file, mut vanguards_set_file, .. } => {
                if cards.is_empty() {
                    Err(Run::GenerateStylesheetSettings { args, failed, art_handler, set_file, schemes_set_file, vanguards_set_file })
                } else {
                    let card = cards.remove(0);
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
                    Err(Run::AddNextCard {
                        client, args, cards, art_handler, set_file, schemes_set_file, vanguards_set_file,
                        added_cards: added_cards + 1,
                        failed: if result.is_ok() { failed } else { failed + 1 },
                        error: result.err().map(|e| (card.to_string(), format!("{:?}", e), e.to_string()))
                    })
                }
            }
            Run::GenerateStylesheetSettings { args, art_handler, set_file, schemes_set_file, vanguards_set_file, .. } => {
                //TODO generate stylesheet settings
                Err(Run::GenerateFooters { args, art_handler, set_file, schemes_set_file, vanguards_set_file })
            }
            Run::GenerateFooters { args, art_handler, set_file, schemes_set_file, vanguards_set_file } => {
                //TODO generate footers (or move into constructors)
                Err(Run::WriteMain { args, art_handler, set_file, schemes_set_file, vanguards_set_file })
            }
            Run::WriteMain { args, mut art_handler, set_file, schemes_set_file, vanguards_set_file } => {
                match args.output {
                    Output::File(ref path) => {
                        task_try!(set_file.write_to(task_try!(File::create(path).at(path)), &mut art_handler));
                        Err(Run::WriteSchemes { args, art_handler, schemes_set_file, vanguards_set_file })
                    }
                    Output::Stdout => {
                        let mut buf = Cursor::<Vec<_>>::default();
                        task_try!(set_file.write_to(&mut buf, &mut art_handler));
                        Err(Run::CopyMain { args, buf, art_handler, schemes_set_file, vanguards_set_file })
                    }
                }
            }
            Run::CopyMain { args, mut buf, art_handler, schemes_set_file, vanguards_set_file } => {
                task_try!(io::copy(&mut buf, &mut stdout()).at_unknown());
                Err(Run::WriteSchemes { args, art_handler, schemes_set_file, vanguards_set_file })
            }
            Run::WriteSchemes { args: ArgsRegular { schemes_output, vanguards_output, .. }, mut art_handler, schemes_set_file, vanguards_set_file } => {
                if let Some(schemes_output) = schemes_output {
                    task_try!(schemes_output.write_set_file(schemes_set_file, &mut art_handler));
                }
                Err(Run::WriteVanguards { vanguards_output, art_handler, vanguards_set_file })
            }
            Run::WriteVanguards { vanguards_output, mut art_handler, vanguards_set_file } => {
                if let Some(vanguards_output) = vanguards_output {
                    task_try!(vanguards_output.write_set_file(vanguards_set_file, &mut art_handler));
                }
                Ok(Ok(()))
            }
        }
    }
}

pub fn client() -> Result<Client, Error> {
    Ok(Client::builder()
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_static(concat!("magic-set-generator/", env!("CARGO_PKG_VERSION"))));
            headers
        })
        .build()?
    )
}
