use {
    std::{
        cell::RefCell,
        fs::File,
        io::{
            self,
            prelude::*
        },
        path::PathBuf,
        thread,
        time::{
            Duration,
            Instant
        }
    },
    itertools::Itertools as _,
    mtg::card::Card,
    serde::Deserialize,
    url::Url,
    crate::{
        args::ArgsRegular,
        util::Error
    }
};

#[derive(Debug, Deserialize)]
struct ScryfallData {
    artist: String,
    card_faces: Option<Vec<ScryfallCardFace>>,
    image_uris: Option<ScryfallImageUris>
}

#[derive(Debug, Deserialize)]
struct ScryfallCardFace {
    name: String,
    image_uris: Option<ScryfallImageUris>
}

#[derive(Debug, Deserialize)]
struct ScryfallImageUris {
    art_crop: Url
}

enum Image {
    Path(PathBuf),
    ScryfallUrl(Url, String),
    LoreSeekerUrl {
        set_code: String,
        collector_number: String,
        card_name: String
    }
}

impl Image {
    fn scryfall_download(config: &ArtHandlerConfig, url: &Url) -> Result<Box<dyn Read>, Error> {
        match config.scryfall_request(url) {
            Ok(resp) => Ok(Box::new(resp) as Box<dyn Read>),
            Err(e) => Err(Error::from(e))
        }
    }

    fn open(&mut self, config: &ArtHandlerConfig) -> Result<Box<dyn Read>, Error> {
        match self {
            Image::Path(path) => File::open(path)
                .map(|f| Box::new(f) as Box<dyn Read>)
                .map_err(Error::from),
            Image::ScryfallUrl(url, card_name) => {
                let mut resp = Image::scryfall_download(config, url)?;
                if let Some(ref img_dir) = config.scryfall_images.as_ref().or(config.images.as_ref()) {
                    let img_path = img_dir.join(format!("{}.png", card_name));
                    io::copy(&mut resp, &mut File::create(&img_path)?)?;
                    //TODO save artist credit in exif data
                    File::open(img_path).map(|f| Box::new(f) as Box<dyn Read>).map_err(Error::from)
                } else {
                    Ok(Box::new(resp))
                }
            }
            Image::LoreSeekerUrl { set_code, collector_number, card_name } => {
                let mut resp = lore_seeker::get(format!("/art/{}/{}.jpg", set_code, collector_number))
                    .or_else(|_| lore_seeker::get(format!("/art/{}/{}.png", set_code, collector_number)))?;
                if let Some(ref img_dir) = config.lore_seeker_images.as_ref().or(config.images.as_ref()) {
                    let img_path = img_dir.join(format!("{}.jpg", card_name));
                    io::copy(&mut resp, &mut File::create(&img_path)?)?;
                    //TODO save artist credit in exif data, if not already present
                    File::open(img_path).map(|f| Box::new(f) as Box<dyn Read>).map_err(Error::from)
                } else {
                    Ok(Box::new(resp))
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
struct ArtHandlerConfig {
    client: reqwest::Client,
    scryfall_rate_limit: RefCell<Option<Instant>>,
    images: Option<PathBuf>,
    lore_seeker_images: Option<PathBuf>,
    no_images: bool,
    no_lore_seeker_images: bool,
    no_scryfall_images: bool,
    scryfall_images: Option<PathBuf>
}

impl ArtHandlerConfig {
    fn scryfall_request(&self, url: &Url) -> Result<reqwest::Response, reqwest::Error> {
        if let Some(next_request_time) = *self.scryfall_rate_limit.borrow() {
            let now = Instant::now();
            if next_request_time > now {
                thread::sleep(next_request_time - now);
            }
        }
        let result = self.client.get(url.as_str()) //TODO remove this url v1 to v2 compat conversion
            .send()
            .and_then(|resp| resp.error_for_status());
        self.scryfall_rate_limit.replace(Some(Instant::now() + Duration::from_millis(100)));
        result
    }
}

pub(crate) struct ArtHandler {
    set_images: Vec<Image>,
    config: ArtHandlerConfig
}

impl ArtHandler {
    pub(crate) fn new(args: &ArgsRegular, client: reqwest::Client) -> ArtHandler {
        ArtHandler {
            set_images: Vec::default(),
            config: ArtHandlerConfig {
                client,
                scryfall_rate_limit: RefCell::default(),
                images: args.images.clone(),
                lore_seeker_images: args.lore_seeker_images.clone(),
                no_images: args.no_images,
                no_lore_seeker_images: args.no_lore_seeker_images(),
                no_scryfall_images: args.no_scryfall_images(),
                scryfall_images: args.scryfall_images.clone()
            }
        }
    }

    fn add_image(&mut self, image: Image) -> usize {
        self.set_images.push(image);
        self.set_images.len()
    }

    pub(crate) fn open_images(&mut self) -> impl Iterator<Item = Result<Box<dyn Read>, Error>> {
        let config = self.config.clone();
        self.set_images.iter_mut().map(move |img| img.open(&config))
    }

    pub(crate) fn register_image_for(&mut self, card: &Card) -> Option<(usize, Option<String>)> {
        if self.config.no_images { return None; }
        for img_dir in &[&self.config.images, &self.config.scryfall_images, &self.config.lore_seeker_images] {
            if let Some(path) = img_dir {
                for file_ext in &["png", "PNG", "jpg", "JPG", "jpeg", "JPEG"] {
                    let image_path = path.join(format!("{}.{}", normalized_image_name(card), file_ext));
                    if image_path.exists() {
                        return Some((self.add_image(Image::Path(image_path)), None)); //TODO artist from exif
                    }
                }
            }
        }
        if !self.config.no_scryfall_images {
            let mut url = Url::parse("https://api.scryfall.com/cards/named").expect("failed to parse Scryfall API URL");
            url.query_pairs_mut().append_pair("exact", &card.to_string());
            if let Ok(mut resp) = self.config.scryfall_request(&url) {
                if let Ok(scryfall_data) = resp.json::<ScryfallData>() {
                    let artist = scryfall_data.artist;
                    let art_crop = if let Some(image_uris) = scryfall_data.image_uris {
                        Some(image_uris.art_crop)
                    } else if let Some(card_faces) = scryfall_data.card_faces {
                        card_faces.into_iter()
                            .filter(|face| face.name == card.to_string())
                            .filter_map(|face| face.image_uris)
                            .collect_tuple()
                            .map(|(image_uris,)| image_uris.art_crop)
                    } else {
                        None
                    };
                    if let Some(art_crop) = art_crop {
                        return Some((self.add_image(Image::ScryfallUrl(art_crop, card.to_string())), Some(artist)));
                    } //TODO else print error if in verbose mode
                } //TODO else print error if in verbose mode
            }
        }
        if !self.config.no_lore_seeker_images {
            if let Ok((_, results)) = lore_seeker::resolve_query(&format!("!{}", card)) {
                if let Some(((_, url),)) = results.into_iter().collect_tuple() {
                    if let Some(segments) = url.path_segments() {
                        if let Some(("card", set_code, collector_number)) = segments.collect_tuple() {
                            return Some((self.add_image(Image::LoreSeekerUrl {
                                set_code: set_code.into(),
                                collector_number: collector_number.into(),
                                card_name: card.to_string()
                            }), None)); //TODO get artist from Lore Seeker
                            //TODO download from Lore Seeker
                        } //TODO else print error if in verbose mode
                    } //TODO else print error if in verbose mode
                } //TODO else print error if in verbose mode
            } //TODO else print error if in verbose mode
        }
        None
    }
}

fn normalized_image_name(card: &Card) -> String {
    let mut card_name = card.to_string();
    card_name.retain(|c| match c {
        ':' | '"' | '?' => false,
        _ => true
    });
    card_name
}
