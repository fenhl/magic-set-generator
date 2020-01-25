#![deny(rust_2018_idioms, unused, unused_import_braces, unused_qualifications, warnings)]

#![windows_subsystem = "windows"]

use {
    iced::{
        Application,
        Command,
        Element,
        Settings
    },
    json_to_mse::util::Never
};

struct JsonToMse;

impl Application for JsonToMse {
    type Message = Never;

    fn new() -> (JsonToMse, Command<Never>) {
        (JsonToMse, Command::none())
    }

    fn title(&self) -> String {
        format!("JSON to MSE")
    }

    fn update(&mut self, message: Never) -> Command<Never> {
        match message {}
    }

    fn view(&mut self) -> Element<'_, Never> {
        unimplemented!() //TODO
    }
}

fn main() {
    JsonToMse::run(Settings::default());
}
