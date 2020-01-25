#![deny(rust_2018_idioms, unused, unused_import_braces, unused_qualifications, warnings)]

#![windows_subsystem = "windows"]

use {
    iced::{
        Application,
        Command,
        Element,
        Settings,
        widget::*
    },
    reqwest::blocking::Client,
    smart_default::SmartDefault,
    json_to_mse::args::ArgsRegular
};

#[derive(Debug, Clone)]
enum Message {
    Start
}

#[derive(SmartDefault)]
struct JsonToMse {
    args: ArgsRegular,
    #[default(json_to_mse::client().expect("failed to create HTTP client"))]
    client: Client,
    start_button: button::State
}

impl Application for JsonToMse {
    type Message = Message;

    fn new() -> (JsonToMse, Command<Message>) {
        (JsonToMse::default(), Command::none())
    }

    fn title(&self) -> String {
        format!("JSON to MSE")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Start => {
                json_to_mse::run(self.client.clone(), self.args.clone()).expect("failed to convert JSON to MSE");
                Command::none()
            }
        }
    }

    fn view(&mut self) -> Element<'_, Message> {
        Column::new()
            .push(Text::new("settings coming soon™"))
            .push(
                Button::new(&mut self.start_button, Text::new("Convert"))
                    .on_press(Message::Start)
            )
            .into()
    }
}

fn main() {
    JsonToMse::run(Settings::default());
}
