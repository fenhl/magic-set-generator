use {
    std::{
        cell::RefCell,
        collections::HashMap,
        fs::File,
        io::{
            self,
            prelude::*
        },
        path::PathBuf,
        sync::Arc,
        thread,
        time::{
            Duration,
            Instant
        }
    },
    itertools::Itertools as _,
    mtg::card::Card,
    parking_lot::Mutex,
    reqwest::blocking::{
        Client,
        Response
    },
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

enum ImageSource {
    Path(PathBuf),
    ScryfallUrl(Url),
    LoreSeekerUrl {
        set_code: String,
        collector_number: String
    }
}

pub(crate) struct Image {
    pub artist: Option<String>,
    card: Card,
    pub id: usize,
    source: ImageSource
}

impl Image {
    fn path(card: Card, path: PathBuf) -> Image {
        Image {
            card,
            id: 0,
            artist: None,
            source: ImageSource::Path(path)
        }
    }

    fn lore_seeker(card: Card, set_code: &str, collector_number: &str) -> Image {
        Image {
            card,
            id: 0,
            artist: None,
            source: ImageSource::LoreSeekerUrl {
                set_code: set_code.into(),
                collector_number: collector_number.into()
            }
        }
    }

    fn scryfall(card: Card, url: Url, artist: String) -> Image {
        Image {
            card,
            id: 0,
            artist: Some(artist),
            source: ImageSource::ScryfallUrl(url)
        }
    }

    fn filename(&self) -> String {
        normalized_image_name(&self.card)
    }

    fn open(&mut self, config: &ArtHandlerConfig) -> Result<Box<dyn Read>, Error> {
        match self.source {
            ImageSource::Path(ref path) => File::open(path)
                .map(|f| Box::new(f) as Box<dyn Read>)
                .map_err(Error::from),
            ImageSource::ScryfallUrl(ref url) => {
                let mut resp = Image::scryfall_download(config, url)?;
                if let Some(ref img_dir) = config.scryfall_images.as_ref().or(config.images.as_ref()) {
                    let img_path = img_dir.join(format!("{}.png", self.filename()));
                    io::copy(&mut resp, &mut File::create(&img_path)?)?;
                    //TODO save artist credit in exif data
                    File::open(img_path).map(|f| Box::new(f) as Box<dyn Read>).map_err(Error::from)
                } else {
                    Ok(Box::new(resp))
                }
            }
            ImageSource::LoreSeekerUrl { ref set_code, ref collector_number } => {
                let mut resp = lore_seeker::get(format!("/art/{}/{}.jpg", set_code, collector_number))
                    .or_else(|_| lore_seeker::get(format!("/art/{}/{}.png", set_code, collector_number)))?;
                if let Some(ref img_dir) = config.lore_seeker_images.as_ref().or(config.images.as_ref()) {
                    let img_path = img_dir.join(format!("{}.jpg", self.filename()));
                    io::copy(&mut resp, &mut File::create(&img_path)?)?;
                    //TODO save artist credit in exif data, if not already present
                    File::open(img_path).map(|f| Box::new(f) as Box<dyn Read>).map_err(Error::from)
                } else {
                    Ok(Box::new(resp))
                }
            }
        }
    }

    fn scryfall_download(config: &ArtHandlerConfig, url: &Url) -> Result<Box<dyn Read>, Error> {
        match config.scryfall_request(url) {
            Ok(resp) => Ok(Box::new(resp) as Box<dyn Read>),
            Err(e) => Err(Error::from(e))
        }
    }
}

#[derive(Debug, Clone)]
struct ArtHandlerConfig {
    client: Client,
    scryfall_rate_limit: RefCell<Option<Instant>>,
    images: Option<PathBuf>,
    lore_seeker_images: Option<PathBuf>,
    no_images: bool,
    no_lore_seeker_images: bool,
    no_scryfall_images: bool,
    scryfall_images: Option<PathBuf>
}

impl ArtHandlerConfig {
    fn scryfall_request(&self, url: &Url) -> Result<Response, reqwest::Error> {
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

pub struct ArtHandler {
    set_images: HashMap<Card, Arc<Mutex<Image>>>,
    config: ArtHandlerConfig
}

impl ArtHandler {
    pub fn new(args: &ArgsRegular, client: Client) -> ArtHandler {
        ArtHandler {
            set_images: HashMap::default(),
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

    fn add_image(&mut self, mut image: Image) -> Option<Arc<Mutex<Image>>> {
        image.id = self.set_images.len() + 1;
        let card = image.card.clone();
        let image_arc = Arc::new(Mutex::new(image));
        self.set_images.insert(card, image_arc.clone());
        Some(image_arc)
    }

    pub(crate) fn open_images(&mut self) -> impl Iterator<Item = Result<(usize, Box<dyn Read>), Error>> + '_ {
        let config = self.config.clone();
        self.set_images.values().map(move |img| {
            let mut img = img.lock();
            img.open(&config).map(|f| (img.id, f))
        })
    }

    pub(crate) fn register_image_for(&mut self, card: &Card) -> Option<Arc<Mutex<Image>>> {
        if self.config.no_images { return None; }
        if let Some(image) = self.set_images.get(card) { return Some(Arc::clone(image)); }
        for img_dir in &[&self.config.images, &self.config.scryfall_images, &self.config.lore_seeker_images] {
            if let Some(path) = img_dir {
                for file_ext in &["png", "PNG", "jpg", "JPG", "jpeg", "JPEG"] {
                    let image_path = path.join(format!("{}.{}", normalized_image_name(card), file_ext));
                    if image_path.exists() {
                        return self.add_image(Image::path(card.clone(), image_path)); //TODO artist from exif
                    }
                }
            }
        }
        if !self.config.no_scryfall_images {
            let mut url = Url::parse("https://api.scryfall.com/cards/named").expect("failed to parse Scryfall API URL");
            url.query_pairs_mut().append_pair("exact", &card.to_string());
            if let Ok(resp) = self.config.scryfall_request(&url) {
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
                        return self.add_image(Image::scryfall(card.clone(), art_crop, artist));
                    } //TODO else print error if in verbose mode
                } //TODO else print error if in verbose mode
            }
        }
        if !self.config.no_lore_seeker_images {
            if let Ok((_, results)) = lore_seeker::resolve_query(&format!("!{}", card)) {
                if let Some(((_, url),)) = results.into_iter().collect_tuple() {
                    if let Some(segments) = url.path_segments() {
                        if let Some(("card", set_code, collector_number)) = segments.collect_tuple() {
                            return self.add_image(Image::lore_seeker(card.clone(), set_code, collector_number)); //TODO get artist from Lore Seeker
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
