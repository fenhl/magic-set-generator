#![deny(rust_2018_idioms, unused, unused_import_braces, unused_qualifications, warnings)]

#![windows_subsystem = "windows"]

use {
    std::{
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
    save_state: text_input::State
}

impl ArgsState {
    fn view(&mut self) -> Element<'_, Message> {
        Column::new()
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
    OutputChange(String)
}

impl ArgsMessage {
    fn handle(self, args: &mut ArgsState) {
        match self {
            ArgsMessage::OutputChange(new_path) => if new_path.is_empty() {
                args.args.output = Output::Stdout;
            } else {
                args.args.output = Output::File(PathBuf::from(new_path));
            }
        }
    }
}

fn main() {
    JsonToMse::run(Settings::default());
}
