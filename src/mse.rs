use {
    std::{
        fmt,
        io::{
            self,
            prelude::*
        },
        iter::FromIterator,
        ops::{
            AddAssign,
            Index,
            IndexMut
        }
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
            EnchantmentType,
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

impl fmt::Display for MseGame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MseGame::Magic => write!(f, "magic"),
            MseGame::Archenemy => write!(f, "archenemy"),
            MseGame::Vanguard => write!(f, "vanguard")
        }
    }
}

#[derive(Debug)]
pub(crate) enum Data {
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

impl Data {
    fn contains(&self, key: impl ToString) -> bool {
        match self {
            Data::Flat(_) => false,
            Data::Subfile(f) => f.contains(key)
        }
    }

    fn expect_subfile_mut(&mut self, msg: &str) -> &mut DataFile {
        match self {
            Data::Flat(_) => { panic!("{}", msg); }
            Data::Subfile(f) => f
        }
    }

    fn render(&self) -> String {
        match self {
            Data::Flat(text) => text.clone(),
            Data::Subfile(f) => f.render()
        }
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
                ("magic-m15-altered", set_styling_data(args, "m15-altered").into())
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

    pub(crate) fn add_card(&mut self, card: &Card, mse_game: MseGame, args: &ArgsRegular, art_handler: &mut ArtHandler) -> Result<(), Error> {
        let card_data = DataFile::from_card(card, mse_game, args, art_handler);
        if let Some(stylesheet) = card_data.get("stylesheet") {
            let prefixed_stylesheet = format!("{}-{}", mse_game, stylesheet.render());
            if !self["styling"].contains(&prefixed_stylesheet) {
                self["styling"].expect_subfile_mut("found flat set styling data").push(prefixed_stylesheet, set_styling_data(args, &stylesheet.render()));
            }
        }
        self.push("card", card_data);
        Ok(())
    }

    fn from_card(card: &Card, mse_game: MseGame, args: &ArgsRegular, art_handler: &mut ArtHandler) -> DataFile {
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
                Layout::Split { right: alt_part, .. } |
                Layout::Flip { flipped: alt_part, .. } |
                Layout::DoubleFaced { back: alt_part, .. } |
                Layout::Meld { back: alt_part, .. } |
                Layout::Adventure { adventure: alt_part, .. } => if !alt {
                    result += DataFile::from_card(&alt_part, mse_game, args, art_handler);
                }
            }
        }
        // name
        push_alt!("name", card.to_string());
        // mana cost
        if let Some(mana_cost) = card.mana_cost() {
            push_alt!("casting cost", cost_to_mse(mana_cost));
        }
        // image
        if let Some(image) = art_handler.register_image_for(card) {
            let image = image.lock();
            push_alt!("image", format!("image{}", image.id));
            if let Some(ref artist) = image.artist {
                push_alt!("illustrator", artist);
            }
        }
        //TODO frame color & color indicator
        if let Some(indicator) = card.color_indicator() {
            push_alt!("indicator", indicator.canonical_order().into_iter().join(", "));
        }
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
        let mut separated_text_boxes =
            if card.is_leveler()
            || card.type_line() >= CardType::Planeswalker
            || card.type_line() >= EnchantmentType::Saga
            || card.type_line() >= EnchantmentType::Discovery
        { Some(Vec::default()) } else { None };
        if !abilities.is_empty() {
            for ability in &abilities {
                match ability {
                    Ability::Other(text) => { //TODO special handling for loyalty abilities
                        if let Some(ref mut separated_text_boxes) = separated_text_boxes {
                            separated_text_boxes.push(with_mse_symbols(text));
                        } else if text.starts_with("Whenever you roll {CHAOS},") {
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
                    Ability::Chapter { text, .. } => if let Some(ref mut separated_text_boxes) = separated_text_boxes {
                        //TODO arrange chapter symbols, enable/disable Discovery mode?
                        separated_text_boxes.push(with_mse_symbols(text));
                    },
                    Ability::Level { min, max, power, toughness, abilities } => if let Some(ref mut separated_text_boxes) = separated_text_boxes {
                        result.push(format!("level {}", separated_text_boxes.len()), if let Some(max) = max {
                            format!("{}-{}", min, max)
                        } else {
                            format!("{}+", min)
                        });
                        result.push(format!("power {}", separated_text_boxes.len() + 1), power);
                        result.push(format!("toughness {}", separated_text_boxes.len() + 1), toughness);
                        separated_text_boxes.push(ability_lines(abilities).join("\n"));
                    }
                    ability => if let Some(ref mut separated_text_boxes) = separated_text_boxes {
                        separated_text_boxes.push(ability_lines(&[ability.clone()]).join("\n"));
                    }
                }
            }
            if let Some(ref separated_text_boxes) = separated_text_boxes {
                for (i, text_box) in separated_text_boxes.iter().enumerate() {
                    result.push(
                        if i == 0 && card.is_leveler() {
                            format!("rule text")
                        } else {
                            format!("level {} text", i + 1)
                        },
                        text_box
                    );
                }
            } else {
                push_alt!("rule text", ability_lines(&abilities).join("\n"));
            }
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
                            "m15-mainframe-planes"
                        } else if card.type_line() >= EnchantmentType::Saga || card.type_line() >= EnchantmentType::Discovery {
                            "m15-saga"
                        } else if card.type_line() >= CardType::Planeswalker {
                            "m15-mainframe-planeswalker"
                        } else if card.is_leveler() {
                            "m15-leveler"
                        } else if card.type_line() >= CardType::Conspiracy {
                            "m15-ttk-conspiracy"
                        } else {
                            "m15-altered"
                        }
                    }
                    Layout::Split { right, .. } => if right.abilities().into_iter().any(|abil| abil == KeywordAbility::Aftermath) {
                        "m15-aftermath"
                    } else {
                        "m15-split-fusable"
                    },
                    Layout::Flip { .. } => "m15-flip",
                    Layout::DoubleFaced { .. } => "m15-mainframe-dfc",
                    Layout::Meld { .. } => "m15-mainframe-dfc",
                    Layout::Adventure { .. } => "m15-flip" //TODO use adventure frame
                },
                MseGame::Archenemy => "standard",
                MseGame::Vanguard => "standard"
            };
            if stylesheet != if mse_game == MseGame::Magic { "m15-altered" } else { "standard" } {
                result.push("stylesheet", stylesheet);
            }
            // stylesheet options
            match stylesheet {
                "m15-altered" => {
                    if card.type_line() >= CardType::Enchantment && card.type_line().types().iter().filter(|&&card_type| card_type != CardType::Tribal).count() >= 2 {
                        result.push_styling(args, stylesheet, "frames", "nyx");
                    }
                    if card.color_indicator().is_some() {
                        result.push_styling(args, stylesheet, "color indicator dot", "yes");
                    }
                }
                "m15-mainframe-dfc" => {
                    let back = match card.layout() {
                        Layout::DoubleFaced { back, .. } |
                        Layout::Meld { back, .. } => back,
                        layout => { panic!("unexpected layout for m15-mainframe-dfc: {:?}", layout); }
                    };
                    if card.type_line() >= CardType::Planeswalker {
                        let num_text_boxes = match separated_text_boxes {
                            Some(boxes) => boxes.len(),
                            None => 3 //TODO verbose warning
                        };
                        result.push_styling(args, stylesheet, "front style", format!("{} ability planeswalker", num_text_boxes));
                    }
                    if back.type_line() >= CardType::Planeswalker {
                        let num_text_boxes = 3; //TODO
                        result.push_styling(args, stylesheet, "back style", format!("{} ability planeswalker", num_text_boxes));
                    }
                }
                "m15-mainframe-planeswalker" => {
                    if card.color_indicator().is_some() {
                        result.push_styling(args, stylesheet, "color indicator dot", "yes");
                    }
                    let num_text_boxes = match separated_text_boxes {
                        Some(boxes) => boxes.len(),
                        None => 3 //TODO verbose warning
                    };
                    result.push_styling(args, stylesheet, "use separate textboxes", match num_text_boxes {
                        2 => "two",
                        3 => "three",
                        4 => "four",
                        _ => "three" //TODO verbose warning
                    });
                }
                _ => {}
            }
        }
        result
    }

    fn contains(&self, key: impl ToString) -> bool {
        let key = key.to_string();
        self.items.iter().any(|(k, _)| *k == key)
    }

    fn get(&self, key: impl ToString) -> Option<&Data> {
        let key = key.to_string();
        for (k, v) in &self.items {
            if *k == key { return Some(v); }
        }
        None
    }

    fn push(&mut self, key: impl ToString, value: impl Into<Data>) {
        self.items.push((key.to_string(), value.into()));
    }

    fn push_styling(&mut self, args: &ArgsRegular, stylesheet: &str, key: impl ToString, value: impl Into<Data>) {
        if !self.contains("styling data") {
            self.push("has styling", "true");
            self.push("styling data", set_styling_data(args, stylesheet));
        }
        match &mut self["styling data"] {
            Data::Flat(text) => { panic!("found flat styling data: {:?}", text); }
            Data::Subfile(f) => { f.push(key, value); }
        }
    }

    fn render(&self) -> String {
        let mut buf = Vec::default();
        self.write_inner(&mut buf, 0).expect("failed to render MSE data file");
        String::from_utf8(buf).expect("MSE data file is not valid UTF-8")
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
                    write!(buf, "{}:\r\n", key)?;
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
        for result in art_handler.open_images() {
            let (i, mut image) = result?;
            zip.start_file(format!("image{}", i), FileOptions::default())?;
            io::copy(&mut image, &mut zip)?;
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

impl<K: ToString> Index<K> for DataFile {
    type Output = Data;

    fn index(&self, index: K) -> &Data {
        let key = index.to_string();
        for (k, v) in &self.items {
            if *k == key {
                return v;
            }
        }
        panic!("data file has no entry with key {}", key);
    }
}

impl<K: ToString> IndexMut<K> for DataFile {
    fn index_mut(&mut self, index: K) -> &mut Data {
        let key = index.to_string();
        for (k, v) in &mut self.items {
            if *k == key {
                return v;
            }
        }
        panic!("data file has no entry with key {}", key);
    }
}

fn ability_lines(abilities: &[Ability]) -> Vec<String> {
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
        ManaSymbol::Runic => format!("V"),
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

fn set_styling_data(args: &ArgsRegular, stylesheet: &str) -> DataFile {
    match stylesheet {
        "m15-altered" => DataFile::from_iter(vec![
            ("other options", Data::from("brown legendary vehicle pt, ancestral generic mana")),
            ("use holofoil stamps", Data::from(if args.holofoil_stamps { "yes" } else { "no" })),
            ("center text", Data::from("short text only"))
        ]),
        "m15-mainframe-dfc" => DataFile::from_iter(vec![
            ("other options", Data::from(format!("use hovering pt, ancestral generic mana{}", if args.holofoil_stamps { ", use holofoil stamps" } else { "" })))
        ]),
        "m15-mainframe-planeswalker" => DataFile::from_iter(vec![
            ("use separate textboxes", Data::from("three")),
            ("other options", Data::from("ancestral generic mana")),
            ("holofoil stamped rares", Data::from(if args.holofoil_stamps { "yes" } else { "no" }))
        ]),
        _ => DataFile::default()
    }
}

fn symbols_to_mse(text: &str) -> String {
    match text {
        "{CHAOS}" => format!("chaos"),
        "{DISCOVER}" => format!("D"), // The {DISCOVER} symbol doesn't exist in the text box symbol font, use this instead to avoid panicking
        "{P}" => format!("phi"),
        "{Q}" => format!("Q"),
        "{T}" => format!("T"),
        _ => if let Ok(mana_cost) = text.parse() {
            cost_to_mse(mana_cost)
        } else if Regex::new("^(\\{E\\})+$").expect("failed to compile energy regex").is_match(text) {
            "E".repeat(text.len() / 3)
        } else {
            panic!("unrecognized symbol: {}", text);
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
