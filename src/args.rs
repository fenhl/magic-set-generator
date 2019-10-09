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
    crate::{
        mse::DataFile,
        util::Error
    }
};
#[cfg(not(windows))]
use {
    std::io::stdin,
    termion::is_tty
};

//TODO !tappedout command
const COMMANDS: [(&str, usize, fn(&mut ArgsRegular, Vec<String>) -> Result<(), Error>); 1] = [
    ("all", 0, command_all)
];

//TODO add remaining flags/options from readme
const FLAGS: [(&str, Option<char>, fn(&mut ArgsRegular) -> Result<(), Error>); 7] = [
    ("include-planes", None, include_planes_on),
    ("include-schemes", None, include_schemes_on),
    ("include-vanguards", None, include_vanguards_on),
    ("no-include-planes", None, include_planes_off),
    ("no-include-schemes", None, include_schemes_off),
    ("no-include-vanguards", None, include_vanguards_off),
    ("verbose", Some('v'), verbose)
];

const OPTIONS: [(&str, Option<char>, fn(&mut ArgsRegular, &str) -> Result<(), Error>); 2] = [
    ("input", Some('i'), input),
    ("output", Some('o'), output)
];

pub(crate) enum Output {
    File(PathBuf),
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
    pub(crate) fn write_set_file(self, set_file: DataFile) -> Result<(), Error> {
        match self {
            Output::File(path) => {
                set_file.write_to(File::create(path)?)?;
            }
            Output::Stdout => {
                let mut buf = Cursor::<Vec<_>>::default();
                set_file.write_to(&mut buf)?;
                io::copy(&mut buf, &mut stdout())?;
            }
        }
        Ok(())
    }
}

pub(crate) struct ArgsRegular {
    pub(crate) all_command: bool,
    pub(crate) auto_card_numbers: bool,
    pub(crate) cards: BTreeSet<String>,
    pub(crate) copyright: String,
    include_planes: Option<bool>,
    include_schemes: Option<bool>,
    include_vanguards: Option<bool>,
    pub(crate) output: Output,
    pub(crate) planes_output: Option<Output>,
    pub(crate) schemes_output: Option<Output>,
    pub(crate) set_code: String,
    pub(crate) vanguards_output: Option<Output>,
    pub(crate) verbose: bool
}

impl Default for ArgsRegular {
    fn default() -> ArgsRegular {
        ArgsRegular {
            all_command: false,
            auto_card_numbers: false,
            cards: BTreeSet::default(),
            copyright: format!("NOT FOR SALE"),
            include_planes: None,
            include_schemes: None,
            include_vanguards: None,
            output: Output::Stdout,
            planes_output: None,
            schemes_output: None,
            set_code: format!("PROXY"),
            vanguards_output: None,
            verbose: false
        }
    }
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
                Err(Error::Args(format!("unknown option in stdin: {}", line)))
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

    pub(crate) fn include_planes(&self) -> bool {
        self.include_planes.unwrap_or(self.planes_output.is_none())
    }

    pub(crate) fn include_schemes(&self) -> bool {
        self.include_schemes.unwrap_or(self.schemes_output.is_none())
    }

    pub(crate) fn include_vanguards(&self) -> bool {
        self.include_vanguards.unwrap_or(self.vanguards_output.is_none())
    }
}

pub(crate) enum Args {
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
    pub(crate) fn new() -> Result<Args, Error> {
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

fn command_all(args: &mut ArgsRegular, _: Vec<String>) -> Result<(), Error> {
    args.all_command = true;
    Ok(())
}

fn include_planes_off(args: &mut ArgsRegular) -> Result<(), Error> {
    args.include_planes = Some(false);
    Ok(())
}

fn include_planes_on(args: &mut ArgsRegular) -> Result<(), Error> {
    args.include_planes = Some(true);
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

fn output(args: &mut ArgsRegular, out_path: &str) -> Result<(), Error> {
    args.output = out_path.parse()?;
    Ok(())
}

fn verbose(args: &mut ArgsRegular) -> Result<(), Error> {
    args.verbose = true;
    Ok(())
}
