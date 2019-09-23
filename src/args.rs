use {
    std::{
        collections::BTreeSet,
        env,
        io::prelude::*
    },
    crate::Error
};

pub(crate) trait WriteSeek: Write + Seek {}

impl<T: Write + Seek> WriteSeek for T {}

pub(crate) struct ArgsRegular {
    pub(crate) all_command: bool,
    pub(crate) auto_card_numbers: bool,
    pub(crate) cards: BTreeSet<String>,
    pub(crate) copyright: String,
    pub(crate) output: Option<Box<dyn WriteSeek>>,
    pub(crate) planes_output: Option<Box<dyn WriteSeek>>,
    pub(crate) schemes_output: Option<Box<dyn WriteSeek>>,
    pub(crate) set_code: String,
    pub(crate) vanguards_output: Option<Box<dyn WriteSeek>>,
    pub(crate) verbose: bool
}

impl Default for ArgsRegular {
    fn default() -> ArgsRegular {
        ArgsRegular {
            all_command: false,
            auto_card_numbers: false,
            cards: BTreeSet::default(),
            copyright: format!("NOT FOR SALE"),
            output: None,
            planes_output: None,
            schemes_output: None,
            set_code: format!("PROXY"),
            vanguards_output: None,
            verbose: false
        }
    }
}

pub(crate) enum Args {
    Regular(ArgsRegular),
    Help,
    Version
}

impl Args {
    pub(crate) fn new() -> Result<Args, Error> {
        let mut args = ArgsRegular::default();
        for arg in env::args().skip(1) {
            if arg.starts_with('-') {
                //TODO options (no stdin support since pos args aren't paths/files)
                if arg.starts_with("--") {
                    if arg == "--help" {
                        return Ok(Args::Help);
                    } else if arg == "--version" {
                        return Ok(Args::Version);
                    } else {
                        return Err(Error::Args);
                    }
                } else {
                    for (_, short_flag) in arg.chars().skip(1).enumerate() {
                        match short_flag {
                            'h' => { return Ok(Args::Help); }
                            _ => { return Err(Error::Args); }
                        }
                    }
                }
            } else {
                //TODO commands, comments, queries
                args.cards.insert(arg);
            }
        }
        Ok(Args::Regular(args))
    }
}
