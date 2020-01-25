use {
    std::{
        collections::BTreeSet,
        env,
        fs::File,
        io::{
            self,
            BufReader,
            Cursor,
            prelude::*,
            stdout
        },
        path::PathBuf,
        str::FromStr
    },
    css_color_parser::Color,
    smart_default::SmartDefault,
    crate::{
        art::ArtHandler,
        mse::DataFile,
        util::Error
    }
};
#[cfg(not(windows))] use {
    std::io::stdin,
    termion::is_tty
};

//TODO !tappedout command
const COMMANDS: [(&str, usize, fn(&mut ArgsRegular, Vec<String>) -> Result<(), Error>); 1] = [
    ("all", 0, command_all)
];

//TODO add remaining flags/options from readme
const FLAGS: [(&str, Option<char>, fn(&mut ArgsRegular) -> Result<(), Error>); 11] = [
    ("auto-card-numbers", None, auto_card_numbers),
    ("holofoil-stamps", None, holofoil_stamps),
    ("include-schemes", None, include_schemes_on),
    ("include-vanguards", None, include_vanguards_on),
    ("no-images", None, no_images),
    ("no-include-schemes", None, include_schemes_off),
    ("no-include-vanguards", None, include_vanguards_off),
    ("no-lore-seeker-images", None, no_lore_seeker_images),
    ("no-scryfall-images", None, no_scryfall_images),
    ("offline", None, offline),
    ("verbose", Some('v'), verbose)
];

const OPTIONS: [(&str, Option<char>, fn(&mut ArgsRegular, &str) -> Result<(), Error>); 11] = [
    ("border", Some('b'), border),
    ("copyright", None, copyright),
    ("db", None, database),
    ("images", None, images),
    ("input", Some('i'), input),
    ("lore-seeker-images", None, lore_seeker_images),
    ("output", Some('o'), output),
    ("schemes-output", None, schemes_output),
    ("scryfall-images", None, scryfall_images),
    ("set-code", None, set_code),
    ("vanguards-output", None, vanguards_output)
];

#[derive(SmartDefault, Clone)]
pub enum Output {
    File(PathBuf),
    #[default]
    Stdout
}

impl FromStr for Output {
    type Err = Error;

    fn from_str(s: &str) -> Result<Output, Error> {
        Ok(if s == "=" {
            Output::Stdout
        } else {
            Output::File(s.parse()?)
        })
    }
}

impl Output {
    pub fn write_set_file(self, set_file: DataFile, art_handler: &mut ArtHandler) -> Result<(), Error> {
        match self {
            Output::File(path) => {
                set_file.write_to(File::create(path)?, art_handler)?;
            }
            Output::Stdout => {
                let mut buf = Cursor::<Vec<_>>::default();
                set_file.write_to(&mut buf, art_handler)?;
                io::copy(&mut buf, &mut stdout())?;
            }
        }
        Ok(())
    }
}

#[derive(SmartDefault, Clone)]
pub struct ArgsRegular {
    pub all_command: bool,
    pub auto_card_numbers: bool,
    #[default(Color { r: 222, g: 127, b: 50, a: 1.0 })]
    pub border_color: Color,
    pub cards: BTreeSet<String>,
    #[default = "NOT FOR SALE"]
    pub copyright: String,
    pub database: Option<PathBuf>,
    pub holofoil_stamps: bool,
    pub images: Option<PathBuf>,
    include_schemes: Option<bool>,
    include_vanguards: Option<bool>,
    pub lore_seeker_images: Option<PathBuf>,
    pub no_images: bool,
    no_lore_seeker_images: bool,
    no_scryfall_images: bool,
    pub offline: bool,
    pub output: Output,
    pub schemes_output: Option<Output>,
    pub scryfall_images: Option<PathBuf>,
    #[default = "PROXY"]
    pub set_code: String,
    pub vanguards_output: Option<Output>,
    pub verbose: bool
}

impl ArgsRegular {
    fn handle_line(&mut self, line: String) -> Result<(), Error> {
        let line = line.trim();
        if line.is_empty() {
            Ok(())
        } else if line.starts_with('-') {
            // no stdin support since pos args aren't paths/files
            if line.starts_with("--") {
                // no “end of options” support since card names can't start with -
                for (long, _, handler) in &FLAGS {
                    if line == format!("--{}", long) {
                        handler(self)?;
                        return Ok(());
                    }
                }
                for (long, _, handler) in &OPTIONS {
                    if line.starts_with(&format!("--{} ", long)) || line.starts_with(&format!("--{}=", long)) {
                        handler(self, &line[format!("--{} ", long).len()..])?;
                        return Ok(());
                    }
                }
                Err(Error::Args(format!("unknown option in stdin or input file: {}", line)))
            } else {
                'short_flags: for (i, short_flag) in line.chars().enumerate().skip(1) {
                    for &(_, short, handler) in &FLAGS {
                        if let Some(short) = short {
                            if short_flag == short {
                                handler(self)?;
                                continue 'short_flags;
                            }
                        }
                    }
                    for &(_, short, handler) in &OPTIONS {
                        if let Some(short) = short {
                            if short_flag == short {
                                handler(self, &line.chars().skip(i + 1).collect::<String>())?;
                                break 'short_flags;
                            }
                        }
                    }
                    return Err(Error::Args(format!("unknown option: -{}", short_flag)));
                }
                Ok(())
            }
        } else if line.starts_with('!') {
            let mut args = shlex::split(&line[1..]).ok_or(Error::Args(format!("failed to split !command line")))?;
            let cmd_name = args.remove(0);
            for &(iter_cmd, num_args, handler) in &COMMANDS {
                if cmd_name == iter_cmd {
                    if args.len() != num_args { return Err(Error::Args(format!("wrong number of !{} arguments: expected {}, got {}", cmd_name, num_args, args.len()))); }
                    handler(self, args)?;
                    return Ok(());
                }
            }
            Err(Error::Args(format!("unknown command: !{}", cmd_name)))
        } else if line.starts_with('#') {
            Ok(()) // comment line
        } else {
            //TODO queries
            self.cards.insert(line.into());
            Ok(())
        }
    }

    pub fn include_schemes(&self) -> bool {
        self.include_schemes.unwrap_or(self.schemes_output.is_none())
    }

    pub fn include_vanguards(&self) -> bool {
        self.include_vanguards.unwrap_or(self.vanguards_output.is_none())
    }

    pub(crate) fn no_lore_seeker_images(&self) -> bool {
        self.offline || self.no_lore_seeker_images
    }

    pub(crate) fn no_scryfall_images(&self) -> bool {
        self.offline || self.no_scryfall_images
    }
}

pub enum Args {
    Regular(ArgsRegular),
    Help,
    Update,
    Version
}

enum HandleShortArgResult {
    Continue,
    Break,
    NoMatch
}

impl Args {
    pub fn new() -> Result<Args, Error> {
        let mut raw_args = env::args().skip(1);
        let mut args = ArgsRegular::default();
        while let Some(arg) = raw_args.next() {
            if arg.starts_with('-') {
                // no stdin support since pos args aren't paths/files
                if arg.starts_with("--") {
                    if Args::handle_long_arg(&arg, &mut raw_args, &mut args)? {
                        // handled
                    } else if arg == "--help" {
                        return Ok(Args::Help);
                    } else if arg == "--update" {
                        return Ok(Args::Update);
                    } else if arg == "--version" {
                        return Ok(Args::Version);
                    } else {
                        return Err(Error::Args(format!("unknown option: {}", arg)));
                    }
                } else {
                    for (i, short_flag) in arg.chars().enumerate().skip(1) {
                        match Args::handle_short_arg(short_flag, &arg.chars().skip(i + 1).collect::<String>(), &mut raw_args, &mut args)? {
                            HandleShortArgResult::Continue => continue,
                            HandleShortArgResult::Break => break,
                            HandleShortArgResult::NoMatch => match short_flag {
                                'h' => { return Ok(Args::Help); }
                                c => { return Err(Error::Args(format!("unknown option: -{}", c))); }
                            }
                        }
                    }
                }
            } else if arg.starts_with('!') {
                let cmd_name = &arg[1..];
                let mut found = false;
                for &(iter_cmd, num_args, handler) in &COMMANDS {
                    if cmd_name == iter_cmd {
                        found = true;
                        let cmd_args = raw_args.by_ref().take(num_args).collect::<Vec<_>>();
                        if cmd_args.len() != num_args { return Err(Error::Args(format!("wrong number of !{} arguments: expected {}, got {}", cmd_name, num_args, cmd_args.len()))); }
                        handler(&mut args, cmd_args)?;
                    }
                }
                if !found {
                    return Err(Error::Args(format!("unknown command: !{}", cmd_name)));
                }
            } else if arg.starts_with('#') {
                // comment arg
            } else {
                //TODO queries
                args.cards.insert(arg);
            }
        }
        #[cfg(not(windows))] { //TODO enable for Windows when https://gitlab.redox-os.org/redox-os/termion/issues/167 is fixed
            let stdin = stdin();
            if !is_tty(&stdin) {
                // also read card names/commands from stdin
                loop {
                    let mut buf = String::default();
                    if stdin.read_line(&mut buf)? == 0 { break; }
                    args.handle_line(buf)?;
                }
            }
        }
        Ok(Args::Regular(args))
    }

    fn handle_long_arg(arg: &str, raw_args: &mut impl Iterator<Item = String>, args: &mut ArgsRegular) -> Result<bool, Error> {
        for (long, _, handler) in &FLAGS {
            if arg == format!("--{}", long) {
                handler(args)?;
                return Ok(true);
            }
        }
        for (long, _, handler) in &OPTIONS {
            if arg == format!("--{}", long) {
                let value = raw_args.next().ok_or(Error::Args(format!("missing value for option: --{}", long)))?;
                handler(args, &value)?;
                return Ok(true);
            }
            let prefix = format!("--{}=", long);
            if arg.starts_with(&prefix) {
                let value = &arg[prefix.len()..];
                handler(args, value)?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn handle_short_arg(short_flag: char, remaining_arg: &str, raw_args: &mut impl Iterator<Item = String>, args: &mut ArgsRegular) -> Result<HandleShortArgResult, Error> {
        for &(_, short, handler) in &FLAGS {
            if let Some(short) = short {
                if short_flag == short {
                    handler(args)?;
                    return Ok(HandleShortArgResult::Continue);
                }
            }
        }
        for &(_, short, handler) in &OPTIONS {
            if let Some(short) = short {
                if short_flag == short {
                    if remaining_arg.is_empty() {
                        handler(args, &raw_args.next().ok_or(Error::Args(format!("missing value for option: -{}", short_flag)))?)?;
                    } else {
                        handler(args, remaining_arg)?;
                    };
                    return Ok(HandleShortArgResult::Break);
                }
            }
        }
        Ok(HandleShortArgResult::NoMatch)
    }
}

fn auto_card_numbers(args: &mut ArgsRegular) -> Result<(), Error> {
    args.auto_card_numbers = true;
    Ok(())
}

fn border(args: &mut ArgsRegular, border_color: &str) -> Result<(), Error> {
    args.border_color = match border_color {
        "b" | "black" => Color { r: 0, g: 0, b: 0, a: 1.0 },
        "w" | "white" => Color { r: 255, g: 255, b: 255, a: 1.0 },
        "s" | "silver" => Color { r: 128, g: 128, b: 128, a: 1.0 },
        "g" | "gold" => Color { r: 200, g: 180, b: 0, a: 1.0 },
        "bronze" => Color { r: 222, g: 127, b: 50, a: 1.0 },
        col => col.parse()?
    };
    Ok(())
}

fn command_all(args: &mut ArgsRegular, _: Vec<String>) -> Result<(), Error> {
    args.all_command = true;
    Ok(())
}

fn copyright(args: &mut ArgsRegular, copyright_text: &str) -> Result<(), Error> {
    args.copyright = copyright_text.into();
    Ok(())
}

fn database(args: &mut ArgsRegular, db_path: &str) -> Result<(), Error> {
    args.database = Some(db_path.into());
    Ok(())
}

fn holofoil_stamps(args: &mut ArgsRegular) -> Result<(), Error> {
    args.holofoil_stamps = true;
    Ok(())
}

fn images(args: &mut ArgsRegular, img_dir: &str) -> Result<(), Error> {
    args.images = Some(img_dir.into());
    Ok(())
}

fn include_schemes_off(args: &mut ArgsRegular) -> Result<(), Error> {
    args.include_schemes = Some(false);
    Ok(())
}

fn include_schemes_on(args: &mut ArgsRegular) -> Result<(), Error> {
    args.include_schemes = Some(true);
    Ok(())
}

fn include_vanguards_off(args: &mut ArgsRegular) -> Result<(), Error> {
    args.include_vanguards = Some(false);
    Ok(())
}

fn include_vanguards_on(args: &mut ArgsRegular) -> Result<(), Error> {
    args.include_vanguards = Some(true);
    Ok(())
}

fn input(args: &mut ArgsRegular, in_path: &str) -> Result<(), Error> {
    BufReader::new(File::open(in_path)?)
        .lines()
        .map(|line| line.map_err(Error::from).and_then(|line| args.handle_line(line)))
        .collect::<Result<_, _>>()?;
    Ok(())
}

fn lore_seeker_images(args: &mut ArgsRegular, img_dir: &str) -> Result<(), Error> {
    args.lore_seeker_images = Some(img_dir.into());
    Ok(())
}

fn no_images(args: &mut ArgsRegular) -> Result<(), Error> {
    args.no_images = true;
    Ok(())
}

fn no_lore_seeker_images(args: &mut ArgsRegular) -> Result<(), Error> {
    args.no_lore_seeker_images = true;
    Ok(())
}

fn no_scryfall_images(args: &mut ArgsRegular) -> Result<(), Error> {
    args.no_scryfall_images = true;
    Ok(())
}

fn offline(args: &mut ArgsRegular) -> Result<(), Error> {
    args.offline = true;
    Ok(())
}

fn output(args: &mut ArgsRegular, out_path: &str) -> Result<(), Error> {
    args.output = out_path.parse()?;
    Ok(())
}

fn schemes_output(args: &mut ArgsRegular, out_path: &str) -> Result<(), Error> {
    args.schemes_output = Some(out_path.parse()?);
    Ok(())
}

fn scryfall_images(args: &mut ArgsRegular, img_dir: &str) -> Result<(), Error> {
    args.scryfall_images = Some(img_dir.into());
    Ok(())
}

fn set_code(args: &mut ArgsRegular, set_code: &str) -> Result<(), Error> {
    args.set_code = set_code.into();
    Ok(())
}

fn vanguards_output(args: &mut ArgsRegular, out_path: &str) -> Result<(), Error> {
    args.vanguards_output = Some(out_path.parse()?);
    Ok(())
}

fn verbose(args: &mut ArgsRegular) -> Result<(), Error> {
    args.verbose = true;
    Ok(())
}
