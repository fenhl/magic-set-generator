#![deny(rust_2018_idioms, unused, unused_import_braces, unused_qualifications, warnings)]

#![windows_subsystem = "windows"]

use {
    std::{
        mem,
        path::PathBuf,
        sync::Arc
    },
    iced::{
        Application,
        Command,
        Element,
        Settings,
        widget::*
    },
    itertools::Itertools as _,
    parking_lot::RwLock,
    reqwest::blocking::Client,
    smart_default::SmartDefault,
    msegen::{
        args::{
            ArgsRegular,
            Output
        },
        version::{
            self,
            UpdateProgress
        }
    }
};

#[derive(Debug, Clone)]
enum Message {
    /// Handled by `ArgsState`
    Args(ArgsMessage),
    /// Sent when the GUI is started.
    Init,
    /// Sent when the user presses the Generate button
    Generate
}

#[derive(SmartDefault)]
struct JsonToMse {
    args: ArgsState,
    #[default(msegen::client().expect("failed to create HTTP client"))]
    client: Client,
    update_progress: Arc<RwLock<UpdateProgress>>,
    start_button: button::State
}

impl Application for JsonToMse {
    type Message = Message;

    fn new() -> (JsonToMse, Command<Message>) {
        (JsonToMse::default(), async { Message::Init }.into())
    }

    fn title(&self) -> String {
        format!("Magic Set Generator")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Args(msg) => {
                msg.handle(&mut self.args);
                Command::none()
            }
            Message::Init => {
                match version::self_update(&self.client) { //TODO make async
                    Ok(Some(new)) => { *self.update_progress.write() = UpdateProgress::RestartToUpdate(new); }
                    Ok(None) => { *self.update_progress.write() = UpdateProgress::NoUpdateAvailable; }
                    Err(e) => { *self.update_progress.write() = UpdateProgress::Error(e); }
                }
                Command::none()
            }
            Message::Generate => {
                msegen::run(self.client.clone(), self.args.args.clone()).expect("failed to generate set file");
                Command::none()
            }
        }
    }

    fn view(&mut self) -> Element<'_, Message> {
        Column::new()
            .push(Text::new(format!("{}", self.update_progress.read())))
            .push(self.args.view())
            .push(
                Button::new(&mut self.start_button, Text::new("Generate"))
                    .on_press(Message::Generate)
            )
            .into()
    }
}

#[derive(Default)]
struct ArgsState {
    args: ArgsRegular,
    show_hide_cards_button: button::State,
    card_delete_buttons: Option<Vec<button::State>>,
    new_card_name: String,
    new_card_state: text_input::State,
    add_card_button: button::State,
    save_state: text_input::State
}

impl ArgsState {
    fn view(&mut self) -> Element<'_, Message> {
        let mut col = Column::new()
            .push(Row::new()
                .push(Text::new(format!("{} card{} added", self.args.cards.len(), if self.args.cards.len() == 1 { "" } else { "s" })))
                .push(Button::new(&mut self.show_hide_cards_button, Text::new(if self.card_delete_buttons.is_some() { "Hide" } else { "Show" })).on_press(Message::Args(ArgsMessage::ShowHideCards)))
            );
        if let Some(ref mut del_btns) = self.card_delete_buttons {
            for (card_name, btn) in self.args.cards.iter().cloned().sorted().zip(del_btns) {
                col = col.push(Row::new().push(Text::new(card_name.clone())).push(Button::new(btn, Text::new("Remove")).on_press(Message::Args(ArgsMessage::RemoveCard(card_name)))));
            }
        }
        col.push(Row::new()
                .push(Text::new("Add card: "))
                .push(TextInput::new(&mut self.new_card_state, "", &self.new_card_name, |new_card| Message::Args(ArgsMessage::NewCardNameChange(new_card))).on_submit(Message::Args(ArgsMessage::AddCard)))
                .push(Button::new(&mut self.add_card_button, Text::new("Add")).on_press(Message::Args(ArgsMessage::AddCard)))
            )
            .push(Row::new().push(Text::new("Save as: ")).push(TextInput::new(&mut self.save_state, "C:\\path\\to\\output.mse-set", &match self.args.output {
                Output::File(ref path) => format!("{}", path.display()),
                Output::Stdout => String::default()
            }, |new_path| Message::Args(ArgsMessage::OutputChange(new_path)))))
            .push(Text::new("more options coming soon™")) //TODO support remaining args
            .into()
    }
}

#[derive(Debug, Clone)]
enum ArgsMessage {
    AddCard,
    NewCardNameChange(String),
    OutputChange(String),
    RemoveCard(String),
    ShowHideCards
}

impl ArgsMessage {
    fn handle(self, args: &mut ArgsState) {
        match self {
            ArgsMessage::AddCard => {
                let new_card_name = mem::take(&mut args.new_card_name);
                if args.args.cards.insert(new_card_name) {
                    if let Some(ref mut btns) = args.card_delete_buttons {
                        btns.push(button::State::default());
                    }
                }
            }
            ArgsMessage::NewCardNameChange(new_card_name) => { args.new_card_name = new_card_name; },
            ArgsMessage::OutputChange(new_path) => if new_path.is_empty() {
                args.args.output = Output::Stdout;
            } else {
                args.args.output = Output::File(PathBuf::from(new_path));
            },
            ArgsMessage::RemoveCard(card_name) => if args.args.cards.remove(&card_name) {
                if let Some(ref mut btns) = args.card_delete_buttons {
                    btns.pop();
                }
            },
            ArgsMessage::ShowHideCards => if args.card_delete_buttons.is_some() {
                args.card_delete_buttons = None;
            } else {
                let mut btns = Vec::default();
                btns.resize_with(args.args.cards.len(), button::State::default);
                args.card_delete_buttons = Some(btns);
            }
        }
    }
}

fn main() {
    JsonToMse::run(Settings::default());
}
