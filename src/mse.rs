use {
    std::{
        io::{
            self,
            prelude::*
        },
        iter::FromIterator,
        ops::AddAssign
    },
    css_color_parser::Color,
    itertools::{
        Itertools as _,
        Position
    },
    mtg::{
        card::{
            Ability,
            Card,
            KeywordAbility,
            Layout,
            Rarity
        },
        cardtype::{
            CardType,
            Subtype
        },
        cost::{
            ManaCost,
            ManaSymbol
        }
    },
    regex::Regex,
    zip::{
        ZipWriter,
        write::FileOptions
    },
    crate::{
        args::ArgsRegular,
        art::ArtHandler,
        util::{
            Error,
            StrExt as _
        }
    }
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MseGame {
    Magic,
    Archenemy,
    Vanguard
}

#[derive(Debug)]
enum Data {
    Flat(String),
    Subfile(DataFile)
}

impl<T: ToString> From<T> for Data {
    fn from(text: T) -> Data {
        Data::Flat(text.to_string())
    }
}

impl From<DataFile> for Data {
    fn from(data_file: DataFile) -> Data {
        Data::Subfile(data_file)
    }
}

impl<K: Into<String>> FromIterator<(K, Data)> for Data {
    fn from_iter<I: IntoIterator<Item = (K, Data)>>(items: I) -> Data {
        Data::Subfile(DataFile::from_iter(items))
    }
}

#[derive(Debug, Default)]
pub(crate) struct DataFile {
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
                ("magic-m15-altered", Data::from_iter(vec![
                    ("other options", Data::from("brown legendary vehicle pt, ancestral generic mana")),
                    ("use holofoil stamps", Data::from(if args.holofoil_stamps { "yes" } else { "no" })),
                    ("center text", Data::from("short text only"))
                ]))
            ]))
        ])
    }

    pub(crate) fn new(args: &ArgsRegular, num_cards: usize) -> DataFile {
        DataFile::new_inner(args, num_cards, "magic", "MTG JSON card import")
    }

    pub(crate) fn new_schemes(args: &ArgsRegular, num_cards: usize) -> DataFile {
        DataFile::new_inner(args, num_cards, "archenemy", "MTG JSON card import: Archenemy schemes")
    }

    pub(crate) fn new_vanguards(args: &ArgsRegular, num_cards: usize) -> DataFile {
        DataFile::new_inner(args, num_cards, "vanguard", "MTG JSON card import: Vanguard avatars")
    }

    pub(crate) fn add_card(&mut self, card: &Card, mse_game: MseGame, art_handler: &mut ArtHandler) -> Result<(), Error> {
        self.push("card", DataFile::from_card(card, mse_game, art_handler));
        //TODO add stylesheet?
        Ok(())
    }

    fn from_card(card: &Card, mse_game: MseGame, art_handler: &mut ArtHandler) -> DataFile {
        let alt = card.is_alt();
        let mut result = DataFile::default();

        macro_rules! push_alt {
            ($key:literal, $val:expr) => {
                if alt {
                    result.push(concat!($key, " 2"), $val);
                } else {
                    result.push($key, $val);
                }
            };
            ($key:expr, $val:expr) => {
                if alt {
                    result.push(format!("{} 2", $key), $val);
                } else {
                    result.push($key, $val);
                }
            };
        }

        // layout & other card parts
        if mse_game == MseGame::Magic {
            match card.layout() {
                Layout::Normal => {} // nothing specific to normal layout
                Layout::Split { right, .. } => if !alt {
                    result += DataFile::from_card(&right, mse_game, art_handler);
                },
                Layout::Flip { flipped, .. } => if !alt {
                    result += DataFile::from_card(&flipped, mse_game, art_handler);
                },
                Layout::DoubleFaced { back, .. } => if !alt {
                    result += DataFile::from_card(&back, mse_game, art_handler);
                },
                Layout::Meld { back, .. } => if !alt {
                    result += DataFile::from_card(&back, mse_game, art_handler);
                },
                Layout::Adventure { .. } => {} //TODO use adventurer template once it's released
            }
        }
        // name
        push_alt!("name", card.to_string());
        // mana cost
        if let Some(mana_cost) = card.mana_cost() {
            push_alt!("casting cost", cost_to_mse(mana_cost));
        }
        // image
        if let Some((image_name, artist)) = art_handler.register_image_for(card) {
            push_alt!("image", image_name);
            if let Some(artist) = artist {
                push_alt!("illustrator", artist);
            }
        }
        //TODO frame color & color indicator
        // type line
        if mse_game == MseGame::Archenemy {
            // Archenemy templates don't have a separate subtypes field, so include them with the card types
            push_alt!("type", card.type_line());
        } else {
            let (supertypes, card_types, subtypes) = card.type_line().parts();
            push_alt!(if mse_game == MseGame::Vanguard { "type" } else { "super type" }, supertypes.into_iter()
                .map(|supertype| format!("<word-list-type>{}</word-list-type>", supertype))
                .chain(card_types.into_iter().map(|card_type| format!("<word-list-type>{}</word-list-type>", card_type)))
                .join(" ")
            );
            push_alt!("sub type", subtypes.into_iter().map(|subtype| {
                let card_type = match subtype {
                    Subtype::Artifact(_) => "artifact",
                    Subtype::Enchantment(_) => "enchantment",
                    Subtype::Land(_) => "land",
                    Subtype::Planeswalker(_) => "planeswalker",
                    Subtype::Spell(_) => "spell",
                    Subtype::Creature(_) => "race",
                    Subtype::Planar(_) => "plane"
                };
                format!("<word-list-{}>{}</word-list-{}>", card_type, subtype, card_type)
            }).join(" "));
        }
        // rarity
        if mse_game != MseGame::Vanguard {
            push_alt!("rarity", match card.rarity() {
                Rarity::Land => "basic land",
                Rarity::Common => "common",
                Rarity::Uncommon => "uncommon",
                Rarity::Rare => "rare",
                Rarity::Mythic => "mythic rare",
                Rarity::Special => "special"
            });
        }
        // text
        //let mut has_miracle = false; //TODO
        //let mut is_draft_matters = false; //TODO
        let abilities = card.abilities();
        if !abilities.is_empty() {
            for ability in &abilities {
                match ability {
                    Ability::Other(text) => { //TODO special handling for loyalty abilities, detect draft-matters
                        if text.starts_with("Whenever you roll {CHAOS},") {
                            result.push("rule text 2", with_mse_symbols(text));
                        } else if Regex::new("\\W[Dd]raft(ed)?\\W").expect("failed to compile draft-matters regex").is_match(text) {
                            //is_draft_matters = true; //TODO
                        }
                    }
                    Ability::Keyword(KeywordAbility::Fuse) => {
                        result.push("rule text 3", "<kw-0><nospellcheck>Fuse</nospellcheck></kw-0>");
                    }
                    Ability::Keyword(KeywordAbility::Miracle(_)) => {
                        //has_miracle = true; //TODO
                    }
                    Ability::Chapter { .. } => {} //TODO chapter symbol handling on Sagas
                    Ability::Level { .. } => {} //TODO level keyword handling on leveler layout
                    _ => {}
                }
            }
            let lines = ability_lines(abilities);
            push_alt!("rule text", lines.join("\n"));
        }
        //TODO layouts and mana symbol watermarks for vanilla cards
        // P/T, loyalty/stability, hand/life modifier
        match mse_game {
            MseGame::Magic => {
                if card.type_line() >= CardType::Planeswalker {
                    if let Some(loyalty) = card.loyalty() {
                        push_alt!("loyalty", loyalty);
                    }
                } else {
                    if let Some((power, toughness)) = card.pt() {
                        push_alt!("power", power);
                        push_alt!("toughness", toughness);
                    } else if let Some(stability) = card.stability() {
                        push_alt!("power", stability);
                    } else if let Some((hand, life)) = card.vanguard_modifiers() {
                        push_alt!("power", hand);
                        push_alt!("toughness", life);
                    }
                }
            }
            MseGame::Archenemy => {}
            MseGame::Vanguard => {
                if let Some((hand, life)) = card.vanguard_modifiers() {
                    push_alt!("handmod", hand);
                    push_alt!("lifemod", life);
                }
            }
        }
        // stylesheet
        if !alt {
            let stylesheet = match mse_game {
                MseGame::Magic => match card.layout() {
                    Layout::Normal => {
                        if card.type_line() >= CardType::Plane || card.type_line() >= CardType::Phenomenon {
                            Some("m15-mainframe-planes")
                        } else if card.type_line() >= CardType::Planeswalker {
                            Some("m15-mainframe-planeswalker")
                        } else if card.is_leveler() {
                            Some("m15-leveler")
                        } else if card.type_line() >= CardType::Conspiracy {
                            Some("m15-ttk-conspiracy")
                        } else {
                            None
                        }
                    }
                    Layout::Split { right, .. } => if right.abilities().into_iter().any(|abil| abil == KeywordAbility::Aftermath) {
                        Some("m15-aftermath")
                    } else {
                        Some("m15-split-fusable")
                    },
                    Layout::Flip { .. } => Some("m15-flip"),
                    Layout::DoubleFaced { .. } => Some("m15-mainframe-dfc"),
                    Layout::Meld { .. } => Some("m15-mainframe-dfc"),
                    Layout::Adventure { .. } => None //TODO
                },
                MseGame::Archenemy => None,
                MseGame::Vanguard => None
            };
            if let Some(stylesheet) = stylesheet {
                result.push("stylesheet", stylesheet);
            }
            //TODO stylesheet options
        }
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

    pub(crate) fn write_to(self, buf: impl Write + Seek, art_handler: &mut ArtHandler) -> Result<(), Error> {
        let mut zip = ZipWriter::new(buf);
        zip.start_file("set", FileOptions::default())?;
        self.write_inner(&mut zip, 0)?;
        for (i, image) in art_handler.open_images().enumerate() {
            zip.start_file(format!("image{}", i + 1), FileOptions::default())?;
            io::copy(&mut image?, &mut zip)?;
        }
        Ok(())
    }
}

impl<K: Into<String>> FromIterator<(K, Data)> for DataFile {
    fn from_iter<I: IntoIterator<Item = (K, Data)>>(items: I) -> DataFile {
        DataFile {
            items: items.into_iter().map(|(k, v)| (k.into(), v)).collect()
        }
    }
}

impl AddAssign for DataFile {
    fn add_assign(&mut self, DataFile { items }: DataFile) {
        self.items.extend(items);
    }
}

fn ability_lines(abilities: Vec<Ability>) -> Vec<String> {
    let mut lines = Vec::default();
    let mut current_keywords = None::<String>;
    for ability in abilities {
        match ability {
            Ability::Keyword(_) => {}
            _ => if let Some(keywords) = current_keywords {
                lines.push(keywords);
                current_keywords = None;
            }
        }
        match ability {
            Ability::Other(text) => { //TODO special handling for loyalty abilities and ability words
                if !text.starts_with("Whenever you roll {CHAOS},") {
                    lines.push(with_mse_symbols(text));
                }
            }
            Ability::Keyword(KeywordAbility::Fuse) => {} // added to rule text 3 by layout handling
            Ability::Keyword(keyword) => { //TODO special handling for fuse, detect miracle
                if let Some(ref mut keywords) = current_keywords {
                    keywords.push_str(&format!(", {}", with_mse_symbols(keyword)));
                } else {
                    current_keywords = Some(with_mse_symbols(keyword.to_string().to_uppercase_first()));
                }
            }
            Ability::Modal { choose, modes } => {
                lines.push(format!("{}<soft-line>", with_mse_symbols(choose)));
                for mode in modes.into_iter().with_position() {
                    lines.push(match mode {
                        Position::Last(mode) | Position::Only(mode) => format!("</soft-line>• {}", with_mse_symbols(mode)),
                        Position::First(mode) | Position::Middle(mode) => format!("</soft-line>• {}<soft-line>", with_mse_symbols(mode))
                    });
                }
            }
            Ability::Chapter { .. } => { lines.push(ability.to_string()); } //TODO chapter symbol handling on Sagas and on other layouts
            Ability::Level { min, max, power, toughness, abilities } => { //TODO level keyword handling on leveler layout
                let level_keyword = if let Some(max) = max {
                    format!("{{LEVEL {}-{}}}", min, max)
                } else {
                    format!("{{LEVEL {}+}}", min)
                };
                if abilities.is_empty() {
                    lines.push(format!("{} {}/{}", level_keyword, power, toughness));
                } else {
                    lines.push(level_keyword);
                    lines.extend(ability_lines(abilities));
                    lines.push(format!("{}/{}", power, toughness));
                }
            }
        }
    }
    if let Some(keywords) = current_keywords {
        lines.push(keywords);
    }
    lines
}

fn cost_to_mse(cost: ManaCost) -> String {
    cost.symbols().into_iter().map(|symbol| match symbol {
        ManaSymbol::Variable => format!("X"),
        ManaSymbol::Generic(n) => n.to_string(),
        ManaSymbol::Snow => format!("S"),
        ManaSymbol::Colorless => format!("C"),
        ManaSymbol::TwobridWhite => format!("2/W"),
        ManaSymbol::TwobridBlue => format!("2/U"),
        ManaSymbol::TwobridBlack => format!("2/B"),
        ManaSymbol::TwobridRed => format!("2/R"),
        ManaSymbol::TwobridGreen => format!("2/G"),
        ManaSymbol::HybridWhiteBlue => format!("W/U"),
        ManaSymbol::HybridBlueBlack => format!("U/B"),
        ManaSymbol::HybridBlackRed => format!("B/R"),
        ManaSymbol::HybridRedGreen => format!("R/G"),
        ManaSymbol::HybridGreenWhite => format!("G/W"),
        ManaSymbol::HybridWhiteBlack => format!("W/B"),
        ManaSymbol::HybridBlueRed => format!("U/R"),
        ManaSymbol::HybridBlackGreen => format!("B/G"),
        ManaSymbol::HybridRedWhite => format!("R/W"),
        ManaSymbol::HybridGreenBlue => format!("G/U"),
        ManaSymbol::PhyrexianWhite => format!("H/W"),
        ManaSymbol::PhyrexianBlue => format!("H/U"),
        ManaSymbol::PhyrexianBlack => format!("H/B"),
        ManaSymbol::PhyrexianRed => format!("H/R"),
        ManaSymbol::PhyrexianGreen => format!("H/G"),
        ManaSymbol::White => format!("W"),
        ManaSymbol::Blue => format!("U"),
        ManaSymbol::Black => format!("B"),
        ManaSymbol::Red => format!("R"),
        ManaSymbol::Green => format!("G")
    }).collect()
}

fn symbols_to_mse(text: &str) -> String {
    match text {
        "{CHAOS}" => format!("chaos"),
        "{P}" => format!("phi"),
        "{Q}" => format!("Q"),
        "{T}" => format!("T"),
        _ => if let Ok(mana_cost) = text.parse() {
            cost_to_mse(mana_cost)
        } else if Regex::new("^(\\{E\\})+$").expect("failed to compile energy regex").is_match(text) {
            "E".repeat(text.len() / 3)
        } else {
            panic!()
        }
    }
}

fn with_mse_symbols(text: impl ToString) -> String {
    let symbols_regex = Regex::new("^([\"']?)(\\{.+\\})([:.,]?[\"']*)$").expect("failed to compile symbols regex");
    let number_regex = Regex::new("^[0-9]+|[XVI]+$").expect("failed to compile number regex");
    text.to_string().split(' ').map(|word| word.split('—').map(|word_part| {
        if let Some(captures) = symbols_regex.captures(word_part) {
            format!("{}<sym>{}</sym>{}", &captures[1], symbols_to_mse(&captures[2]), &captures[3])
        } else if number_regex.is_match(word_part) {
            format!("</sym>{}<sym>", word_part)
        } else {
            word_part.into()
        }
    }).join("—")).join(" ")
}
