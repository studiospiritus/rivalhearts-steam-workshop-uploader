mod err_dialog_types;
mod file_field;
mod item_info;
mod my_steamworks;
use err_dialog_types::ErrorDialogUnwrapper;
use iced::widget::{button, column, row, text, text_input};
use iced::{Application, Command, Element, Settings};
use item_info::{ItemInfo, ItemInfoMessage, ItemInfoState};
use my_steamworks::WorkshopClient;
use std::num::IntErrorKind;
use steamworks::{AppId, PublishedFileId, SteamError};

const APP_ID_STR: &str = include_str!("../steam_appid.txt");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    SetExistingId(String),
    EditItemData(ItemInfoMessage),
    ReceiveFoundItemInfo(ItemInfo),
    ReceiveItemId(PublishedFileId),
    ReceiveSteamError(SteamError),
    Proceed,
    GoBack,
    TermsLinkPressed,
}

impl Message {
    fn receive_item_id(res: Result<(PublishedFileId, bool), SteamError>) -> Self {
        match res {
            Ok((id, _)) => Message::ReceiveItemId(id),
            Err(err) => Message::ReceiveSteamError(err),
        }
    }

    fn receive_item_info(res: Result<ItemInfo, SteamError>) -> Self {
        match res {
            Ok(item_info) => Message::ReceiveFoundItemInfo(item_info),
            Err(err) => Message::ReceiveSteamError(err),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
enum ModelState {
    Initial(String),
    ExistingIdSearching(PublishedFileId, Option<SteamError>),
    ItemForm(Option<PublishedFileId>, ItemInfoState),
    CreatingItem(ItemInfo),
    CreationError(ItemInfo, SteamError),
    SendingItem(PublishedFileId, ItemInfo),
    SendingError(PublishedFileId, ItemInfo, SteamError),
    Done(PublishedFileId),
}

struct Model {
    client: WorkshopClient,
    state: ModelState,
}

fn initial_view<'a>(existing_id: &str) -> Element<'a, Message> {
    let item_id = existing_id.parse::<u64>().map(PublishedFileId);

    let mut res = column![
        text(" "),
        text("Rivalhearts - Steam Workshop Uploader"),
        text(" "),
        text("Upload your Rivalhearts mod to the Steam Workshop with this tool!"),
        text(" "),
        if let Err(error) = &item_id {
            if *error.kind() == IntErrorKind::Empty {
                button("Create new workshop mod").on_press(Message::Proceed)
            } else {
                button("Update workshop mod below")
            }
        } else {
            button("Update workshop mod below").on_press(Message::Proceed)
        },
        text(" "),
        text_input("Or enter existing workshop item ID", existing_id, Message::SetExistingId)
            .on_submit(Message::Proceed),
    ];

    if let Err(error) = item_id {
        if *error.kind() != IntErrorKind::Empty {
            res = res.push(text(format!("Invalid workshop item ID: {}.", error)));
        }
    }

    res.into()
}

fn edit_item_view<'a>(
    item_info: &'a ItemInfoState,
    existing_id: Option<PublishedFileId>,
) -> Element<'a, Message> {
    let ready_info = ItemInfo::try_from(item_info.clone());

    let mut fwd_button = if existing_id.is_some() {
        button("Update mod!")
    } else {
        button("Create mod!")
    };

    if let Ok(_) = &ready_info {
        fwd_button = fwd_button.on_press(Message::Proceed);
    }

    column![
        item_info
            .view(existing_id)
            .map(move |message| Message::EditItemData(message)),
        column![
            text(" "),
            text("Before submitting this mod, you must agree to the Steam Workshop's Terms of Service if you haven't already."),
            text(" "),
            button("Workshop TOS").on_press(Message::TermsLinkPressed),
            text(" ")
        ],
        row![button("Go back").on_press(Message::GoBack),text("     "), fwd_button],
        match ready_info {
            Ok(_) => text(""),
            Err(error) => text(format!("{}", error)),
        },
    ]
    .into()
}

impl Model {
    fn update_to_create_item(&mut self, item_info: ItemInfo) -> Command<Message> {
        self.state = ModelState::CreatingItem(item_info);
        Command::perform(self.client.clone().create_item(), Message::receive_item_id)
    }

    fn update_to_send_item(
        &mut self,
        item_id: PublishedFileId,
        item_info: ItemInfo,
    ) -> Command<Message> {
        self.state = ModelState::SendingItem(item_id, item_info.clone());
        Command::perform(
            self.client.clone().send_item(item_id, item_info),
            Message::receive_item_id,
        )
    }
}

impl Application for Model {
    type Message = Message;
    type Executor = iced::executor::Default;
    type Flags = WorkshopClient;
    type Theme = iced::Theme;

    fn new(client: Self::Flags) -> (Self, Command<Self::Message>) {
        let state = ModelState::Initial(String::new());

        (Model { client, state }, Command::none())
    }

    fn title(&self) -> String {
        String::from("Workshop Uploader For Rivalhearts")
    }

    fn update(&mut self, message: Self::Message) -> Command<Message> {
        const CMDN: Command<Message> = Command::none();

        if std::mem::discriminant(&message) == std::mem::discriminant(&Message::TermsLinkPressed) {
            self.client.open_terms();
            return CMDN;
        }

        match self.state.clone() {
            ModelState::Initial(idstr) => match message {
                Message::SetExistingId(idstr) => {
                    self.state = ModelState::Initial(idstr);
                    CMDN
                }
                Message::Proceed => match idstr.parse::<u64>().map(PublishedFileId) {
                    Ok(item_id) => {
                        self.state = ModelState::ExistingIdSearching(item_id, None);
                        Command::perform(
                            self.client.clone().get_item_info(item_id),
                            Message::receive_item_info,
                        )
                    }
                    _ => {
                        self.state = ModelState::ItemForm(None, ItemInfoState::default());
                        CMDN
                    }
                },
                _ => CMDN,
            },
            ModelState::ExistingIdSearching(item_id, _) => {
                match message {
                    Message::GoBack => self.state = ModelState::Initial(item_id.0.to_string()),
                    Message::ReceiveFoundItemInfo(item_info) => {
                        self.state = ModelState::ItemForm(Some(item_id), item_info.into())
                    }
                    Message::ReceiveSteamError(err) => {
                        self.state = ModelState::ExistingIdSearching(item_id, Some(err))
                    }
                    _ => (),
                };
                CMDN
            }
            ModelState::ItemForm(maybe_id, mut item_info) => match message {
                Message::EditItemData(item_info_message) => {
                    item_info.update(item_info_message);
                    self.state = ModelState::ItemForm(maybe_id, item_info);
                    CMDN
                }
                Message::Proceed => match ItemInfo::try_from(item_info.clone()) {
                    Ok(item_info) => match maybe_id {
                        Some(item_id) => self.update_to_send_item(item_id, item_info),
                        None => self.update_to_create_item(item_info),
                    },
                    Err(error) => {
                        println!("Error: {}", error);
                        CMDN
                    }
                },
                Message::GoBack => {
                    self.state = ModelState::Initial(
                        maybe_id
                            .map(|id| id.0.to_string())
                            .unwrap_or(String::default()),
                    );
                    CMDN
                }
                _ => CMDN,
            },
            ModelState::CreatingItem(item_info) => match message {
                Message::ReceiveItemId(item_id) => self.update_to_send_item(item_id, item_info),
                Message::ReceiveSteamError(err) => {
                    self.state = ModelState::CreationError(item_info, err);
                    CMDN
                }
                _ => CMDN,
            },
            ModelState::CreationError(item_info, _err) => {
                match message {
                    Message::GoBack => self.state = ModelState::ItemForm(None, item_info.into()),
                    _ => (),
                };
                CMDN
            }
            ModelState::SendingItem(item_id, item_info) => {
                match message {
                    Message::ReceiveItemId(incoming_id) => {
                        if incoming_id != item_id {
                            println!(
                                "Not advancing due to non-matching IDs. Expected {}, got {}.",
                                item_id.0, incoming_id.0,
                            );
                        } else {
                            self.state = ModelState::Done(item_id);
                        };
                    }
                    Message::ReceiveSteamError(err) => {
                        self.state = ModelState::SendingError(item_id, item_info, err);
                    }
                    _ => (),
                };
                CMDN
            }
            ModelState::SendingError(item_id, item_info, _err) => {
                match message {
                    Message::GoBack => {
                        self.state = ModelState::ItemForm(item_id.into(), item_info.into())
                    }
                    _ => (),
                };
                CMDN
            }
            ModelState::Done(item_id) => {
                match message {
                    Message::Proceed => {
                        let item_url = format!("steam://url/CommunityFilePage/{}", item_id.0);
                        self.client.open_url(item_url.as_str());
                    }
                    Message::GoBack => {
                        self.state = ModelState::Initial(String::default());
                    }
                    _ => (),
                };
                CMDN
            }
        }
    }

    fn view(&self) -> Element<Self::Message> {
        match &self.state {
            ModelState::Initial(existing_id) => initial_view(existing_id.as_str()),
            ModelState::ExistingIdSearching(item_id, None) => column![
                text(format!("Searching for workshop item with ID {}...", item_id.0)),
                button("Cancel").on_press(Message::GoBack),
            ]
            .into(),
            ModelState::ExistingIdSearching(item_id, Some(e)) => column![
                text(" "),
                text(" "),
                text(format!(
                    "Search for workshop item with ID {} failed.\nError: {:?}",
                    item_id.0, e
                )),
                button("Go Back").on_press(Message::GoBack),
            ]
            .into(),
            ModelState::ItemForm(item_id, item_state) => edit_item_view(item_state, *item_id),
            ModelState::CreatingItem(item_info) => column![
                text(" "),
                text(" "),
                text(" "),
                text(format!("Creating \"{}\" on the Steam Workshop...", item_info.name).as_str())
            ].into(),
            ModelState::CreationError(item_info, err) => column![text(format!(
                "Error creating a new entry on the Steam Workshop:\n{:?}\n\"{}\" was not uploaded.",
                err, item_info.name
            )),
            button("Go Back").on_press(Message::GoBack),
            ]
            .into(),
            ModelState::SendingItem(item_id, _item_info) => column![
                text(" "),
                text(" "),
                text(" "),
                text(format!("Sending mod {} to the Steam Workshop...", item_id.0).as_str())
            ].into(),
            ModelState::SendingError(item_id, item_info, err) => column![text(format!(
                "Error uploading your mod to the workshop:\n{:?}\n\"{}\" is created on the workshop with item ID {}, but does not have this upload's files in it.\nPlease resolve the issue and try uploading to this existing ID again.",
                err, item_info.name, item_id.0
            ).as_str()),
            button("Go Back").on_press(Message::GoBack),
            ].into(),
            ModelState::Done(id) => column![
                text(" "),
                text(" "),
                text(" "),
                text(format!("Item ID {} uploaded to workshop.", id.0)),
                text(" "),
                text("Next, please go to your item on the Steam Workshop.\nBy default it is unlisted, so that it can be further edited.\n\nAdd promotional screenshots and a description there, as this application only takes care of the name, file content, and thumbnail icon.\n\nOnce that is complete, change the visibility to public, so people will be able to view and download your mod!"),
                text(" "),
                row![button("Go to your item").on_press(Message::Proceed), text("   "), button("Restart").on_press(Message::GoBack)],                
            ]
            .into(),
        }
    }
}

fn main() -> iced::Result {
    std::env::set_var("WINIT_UNIX_BACKEND", "x11");
    let client = APP_ID_STR
        .parse()
        .map(AppId)
        .map(WorkshopClient::init_app)
        .expect_or_dialog("Failed to parse App ID. This build of the workshop uploader is corrupt.")
        .expect_or_dialog("Failed to initialize Steam Workshop client.");

    let _icon = match image::load_from_memory(include_bytes!("../assets/icon.ico")) {
        Ok(buffer) => {
            let buffer = buffer.to_rgba8();
            let width = buffer.width();
            let height = buffer.height();
            let dynamic_image = image::DynamicImage::ImageRgba8(buffer);
            match iced::window::icon::Icon::from_rgba(dynamic_image.into_bytes(), width, height) {
                Ok(icon) => Some(icon),
                Err(_) => None,
            }
        }
        Err(_) => None,
    };

    Model::run(Settings {
        id: None,
        window: iced::window::Settings {
            size: (500, 800),
            position: iced::window::Position::Centered,
            min_size: None,
            max_size: None,
            visible: true,
            resizable: true,
            decorations: true,
            transparent: false,
            always_on_top: false,
            icon: _icon,
        },
        flags: client,
        default_font: None,
        default_text_size: 20,
        text_multithreading: false,
        antialiasing: false,
        exit_on_close_request: true,
        try_opengles_first: false,
    })
}
