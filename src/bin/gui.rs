#![deny(rust_2018_idioms, unused, unused_import_braces, unused_qualifications, warnings)]

#![windows_subsystem = "windows"]

use {
    std::{
        mem,
        path::PathBuf,
        sync::Arc
    },
    gres::{
        Percent,
        Progress as _,
        Task as _
    },
    iced::{
        Application,
        Command,
        Element,
        Settings,
        executor,
        widget::*
    },
    itertools::Itertools as _,
    parking_lot::RwLock,
    reqwest::blocking::Client,
    smart_default::SmartDefault,
    msegen::{
        Run,
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
    /// Sent when a set has been generated successfully.
    Done,
    /// Sent when the GUI is started.
    Init,
    /// Sent when an error occurs during set generation.
    GenError(String),
    /// Sent when the user presses the Generate button
    Generate(Option<Run>)
}

#[derive(SmartDefault)]
struct JsonToMse {
    args: ArgsState,
    #[default(msegen::client().expect("failed to create HTTP client"))]
    client: Client,
    update_progress: Arc<RwLock<UpdateProgress>>,
    #[default(Ok(button::State::default()))]
    run: Result<button::State, (Percent, String)>
}

impl Application for JsonToMse {
    type Executor = executor::Default;
    type Message = Message;
    type Flags = ();

    fn new((): ()) -> (JsonToMse, Command<Message>) {
        (JsonToMse::default(), async { Message::Init }.into())
    }

    fn title(&self) -> String {
        format!("Magic Set Generator")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Args(msg) => self.args.handle(msg),
            Message::Done => {
                self.run = Ok(button::State::default());
                Command::none()
            }
            Message::GenError(msg) => {
                self.run = Err((Percent::MAX, format!("failed to generate set file: {}", msg)));
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
            Message::Generate(run) => {
                let run = if let Some(run) = run {
                    self.run = Err((run.progress(), run.to_string()));
                    run
                } else {
                    Run::new(self.client.clone(), self.args.args.clone())
                };
                async {
                    match run.run().await {
                        Ok(Ok(())) => Message::Done,
                        Ok(Err(e)) => Message::GenError(e.to_string()),
                        Err(run) => Message::Generate(Some(run))
                    }
                }.into()
            }
        }
    }

    fn view(&mut self) -> Element<'_, Message> {
        let mut col = Column::new()
            .push(Text::new(format!("{}", self.update_progress.read())))
            .push(self.args.view());
        match self.run {
            Ok(ref mut start_button) => {
                col = col.push(
                    Button::new(start_button, Text::new("Generate"))
                        .on_press(Message::Generate(None))
                );
            }
            Err((_, ref run)) => { col = col.push(Text::new(run)); }
        }
        col.into()
    }
}

#[derive(Debug, Clone)]
enum ArgsMessage {
    NewCardNameChange(String),
    AddCard,
    QueryChange(String),
    Search,
    OutputChange(String),
    RemoveCard(String),
    ShowHideCards
}

#[derive(Default)]
struct ArgsState {
    args: ArgsRegular,
    show_hide_cards_button: button::State,
    card_delete_buttons: Option<Vec<button::State>>,
    new_card_name: String,
    new_card_state: text_input::State,
    add_card_button: button::State,
    query: String,
    query_state: text_input::State,
    query_error: Option<String>,
    run_query_button: button::State,
    save_state: text_input::State
}

impl ArgsState {
    fn handle(&mut self, message: ArgsMessage) -> Command<Message> {
        match message {
            ArgsMessage::NewCardNameChange(new_card_name) => { self.new_card_name = new_card_name; }
            ArgsMessage::AddCard => {
                let new_card_name = mem::take(&mut self.new_card_name);
                if self.args.cards.insert(new_card_name) {
                    if let Some(ref mut btns) = self.card_delete_buttons {
                        btns.push(button::State::default());
                    }
                }
            }
            ArgsMessage::QueryChange(new_query) => { self.query = new_query; }
            ArgsMessage::Search => {
                let query = mem::take(&mut self.query);
                match lore_seeker::resolve_query(None, &query) { //TODO async, allow changing Lore Seeker hostname
                    Ok((_, cards)) => {
                        self.query_error = None;
                        self.args.cards.extend(cards.into_iter().map(|(card_name, _)| card_name));
                        if let Some(ref mut btns) = self.card_delete_buttons {
                            btns.resize_with(self.args.cards.len(), button::State::default);
                        }
                    }
                    Err(e) => { self.query_error = Some(e.to_string()); }
                }
            }
            ArgsMessage::OutputChange(new_path) => if new_path.is_empty() {
                self.args.output = Output::Stdout;
            } else {
                self.args.output = Output::File(PathBuf::from(new_path));
            },
            ArgsMessage::RemoveCard(card_name) => if self.args.cards.remove(&card_name) {
                if let Some(ref mut btns) = self.card_delete_buttons {
                    btns.pop();
                }
            }
            ArgsMessage::ShowHideCards => if self.card_delete_buttons.is_some() {
                self.card_delete_buttons = None;
            } else {
                let mut btns = Vec::default();
                btns.resize_with(self.args.cards.len(), button::State::default);
                self.card_delete_buttons = Some(btns);
            }
        }
        Command::none()
    }

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
        col = col
            .push(Row::new()
                .push(Text::new("Add card: "))
                .push(TextInput::new(&mut self.new_card_state, "", &self.new_card_name, |new_card| Message::Args(ArgsMessage::NewCardNameChange(new_card))).on_submit(Message::Args(ArgsMessage::AddCard)))
                .push(Button::new(&mut self.add_card_button, Text::new("Add")).on_press(Message::Args(ArgsMessage::AddCard)))
            )
            .push(Row::new()
                .push(Text::new("Add cards from Lore Seeker search: "))
                .push(TextInput::new(&mut self.query_state, "", &self.query, |new_query| Message::Args(ArgsMessage::QueryChange(new_query))).on_submit(Message::Args(ArgsMessage::Search)))
                .push(Button::new(&mut self.run_query_button, Text::new("Add")).on_press(Message::Args(ArgsMessage::Search)))
            );
        if let Some(ref msg) = self.query_error {
            col = col.push(Row::new().push(Text::new(msg)));
        }
        col.push(Row::new().push(Text::new("Save as: ")).push(TextInput::new(&mut self.save_state, "C:\\path\\to\\output.mse-set", &match self.args.output {
                Output::File(ref path) => format!("{}", path.display()),
                Output::Stdout => String::default()
            }, |new_path| Message::Args(ArgsMessage::OutputChange(new_path)))))
            .push(Text::new("more options coming soon™")) //TODO support remaining args
            .into()
    }
}

fn main() {
    JsonToMse::run(Settings::default());
}
