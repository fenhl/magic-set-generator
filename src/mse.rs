use {
    std::{
        fs::File,
        io::{
            self,
            prelude::*
        },
        iter::FromIterator,
        path::PathBuf
    },
    css_color_parser::Color,
    derive_more::From,
    mtg::card::{
        Db,
        Card
    },
    zip::{
        ZipWriter,
        write::FileOptions
    },
    crate::{
        args::ArgsRegular,
        util::Error
    }
};

pub(crate) enum MseGame {
    Magic,
    Archenemy,
    Planechase,
    Vanguard
}

#[derive(Debug, From)]
enum Data {
    Flat(String),
    Subfile(DataFile)
}

impl<'a> From<&'a str> for Data {
    fn from(text: &'a str) -> Data {
        Data::Flat(text.to_string())
    }
}


impl<K: Into<String>> FromIterator<(K, Data)> for Data {
    fn from_iter<I: IntoIterator<Item = (K, Data)>>(items: I) -> Data {
        Data::Subfile(DataFile::from_iter(items))
    }
}

#[derive(Debug, Default)]
pub(crate) struct DataFile {
    images: Vec<PathBuf>,
    items: Vec<(String, Data)>
}

impl DataFile {
    fn new_inner(args: &ArgsRegular, num_cards: usize, game: &str, title: &str) -> DataFile {
        let mut set_info = DataFile::from_iter(vec![
            ("title", Data::from(title)),
            ("copyright", Data::from(&args.copyright[..])),
            ("description", Data::from(format!("{} automatically imported from MTG JSON using json-to-mse.", if num_cards == 1 { "This card was" } else { "These cards were" }))),
            ("set code", Data::from(&args.set_code[..])),
            ("set language", Data::from("EN")),
            ("mark errors", Data::from("no")),
            ("automatic reminder text", Data::from(String::default())),
            ("automatic card numbers", Data::from(if args.auto_card_numbers { "yes" } else { "no" })),
            ("mana cost sorting", Data::from("unsorted"))
        ]);
        if args.border_color != (Color { r: 0, g: 0, b: 0, a: 1.0 }) {
            let Color { r, g, b, .. } = args.border_color;
            set_info.push("border color", format!("rgb({}, {}, {})", r, g, b));
        }
        DataFile::from_iter(vec![
            ("mse version", Data::from("0.3.8")),
            ("game", Data::from(game)),
            ("stylesheet", Data::from(if game == "magic" { "m15-altered" } else { "standard" })),
            ("set info", Data::Subfile(set_info)),
            ("styling", Data::from_iter(vec![ // styling needs to be above cards
                ("magic-m15-altered", Data::from_iter(Vec::<(String, Data)>::default())) //TODO
            ]))
        ])
    }

    pub(crate) fn new(args: &ArgsRegular, num_cards: usize) -> DataFile {
        DataFile::new_inner(args, num_cards, "magic", "MTG JSON card import")
    }

    pub(crate) fn new_planes(args: &ArgsRegular, num_cards: usize) -> DataFile {
        DataFile::new_inner(args, num_cards, "planechase", "MTG JSON card import: planes and phenomena")
    }

    pub(crate) fn new_schemes(args: &ArgsRegular, num_cards: usize) -> DataFile {
        DataFile::new_inner(args, num_cards, "archenemy", "MTG JSON card import: Archenemy schemes")
    }

    pub(crate) fn new_vanguards(args: &ArgsRegular, num_cards: usize) -> DataFile {
        DataFile::new_inner(args, num_cards, "vanguard", "MTG JSON card import: Vanguard avatars")
    }

    pub(crate) fn add_card(&mut self, card: &Card, _: &Db, _: MseGame, _: &ArgsRegular) -> Result<(), Error> {
        self.push("card", DataFile::from_card(card));
        //TODO add stylesheet?
        Ok(())
    }

    fn from_card(card: &Card) -> DataFile {
        let mut result = DataFile::default();
        result.push("name", card.to_string()); //TODO alt_key
        //TODO other fields
        result
    }

    fn push(&mut self, key: impl ToString, value: impl Into<Data>) {
        self.items.push((key.to_string(), value.into()));
    }

    fn write_inner(&self, buf: &mut impl Write, indent: usize) -> Result<(), io::Error> {
        for (key, value) in &self.items {
            write!(buf, "{}", "\t".repeat(indent))?;
            match value {
                Data::Flat(text) => {
                    if text.contains('\n') {
                        write!(buf, "{}:\r\n", key)?;
                        for line in text.split('\n') {
                            write!(buf, "{}{}\r\n", "\t".repeat(indent + 1), line)?;
                        }
                    } else {
                        write!(buf, "{}: {}\r\n", key, text)?;
                    }
                }
                Data::Subfile(file) => {
                    write!(buf, "{}\r\n", key)?;
                    file.write_inner(buf, indent + 1)?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn write_to(self, buf: impl Write + Seek) -> io::Result<()> {
        let mut zip = ZipWriter::new(buf);
        zip.start_file("set", FileOptions::default())?;
        self.write_inner(&mut zip, 0)?;
        for (i, image_path) in self.images.into_iter().enumerate() {
            zip.start_file(format!("image{}", i + 1), FileOptions::default())?;
            io::copy(&mut File::open(&image_path)?, &mut zip)?;
        }
        Ok(())
    }
}

impl<K: Into<String>> FromIterator<(K, Data)> for DataFile {
    fn from_iter<I: IntoIterator<Item = (K, Data)>>(items: I) -> DataFile {
        DataFile {
            images: Vec::default(),
            items: items.into_iter().map(|(k, v)| (k.into(), v)).collect()
        }
    }
}
