use super::file_field::FileField;
use iced::widget::{column, text, text_input,checkbox};
use iced::Element;
use std::path::PathBuf;
use steamworks::{PublishedFileId, QueryResult};


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemInfoMessage {
    EditName(String),
    EditPreviewImage(String),
    EditTargetFolder(String),
    BrowsePreviewImage,
    BrowseTargetFolder,
    EditChangeNotes(String),
    AbsToggled(bool),
    ConvertToggled(bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemInfoState {
    name: String,
    preview_image: FileField,
    target_folder: FileField,
    change_notes: String,
    use_abs_path: bool,
    convert_rpy: bool,
}

impl Default for ItemInfoState {
    fn default() -> Self {
        ItemInfoState {
            name: String::new(),
            preview_image: FileField::new(),
            target_folder: FileField::new(),
            change_notes: String::new(),
            use_abs_path: false,
            convert_rpy: true,
        }
    }
}

impl ItemInfoState {
    pub fn update(&mut self, message: ItemInfoMessage) {
        match message {
            ItemInfoMessage::EditName(new_name) => self.name = new_name,
            ItemInfoMessage::EditPreviewImage(new_path) => {
                self.preview_image = FileField::from(new_path)
            }
            ItemInfoMessage::EditTargetFolder(new_path) => {
                self.target_folder = FileField::from(new_path)
            }
            ItemInfoMessage::BrowsePreviewImage => {
                self.preview_image.select_file();
            }
            ItemInfoMessage::BrowseTargetFolder => {
                self.target_folder.select_dir();
            }
            ItemInfoMessage::EditChangeNotes(new_notes) => self.change_notes = new_notes,
            ItemInfoMessage::AbsToggled(value) => self.use_abs_path = value,
            ItemInfoMessage::ConvertToggled(val) => self.convert_rpy = val,
        }
    }

    pub fn view(&self, file_id: Option<PublishedFileId>) -> Element<ItemInfoMessage> {
        column![
            text(" "),
            text(" "),
            if let Some(file_id) = file_id {
                text(format!("Updating item with ID: {}", file_id.0))
            } else {
                text("Creating new item:")
            },
            //if let Some(file_id) = file_id {
            //    text("\nYou can leave an entry below empty to not update it.\n")
            //} else {
            text(" "),
            //},
            text_input("Name", &self.name, ItemInfoMessage::EditName,),
            text(" "),
            self.preview_image.view(
                "Thumbnail Image:\nThis should be a 16:9 image and must be under 1MB.",
                //if file_id.is_some() { "Optional" } else { "" },
                "",
                ItemInfoMessage::EditPreviewImage,
                ItemInfoMessage::BrowsePreviewImage,
            ),
            text(" "),
            self.target_folder.view(
                "Your Mod's Folder:\nFor example \"\\game\\mods\\YourMod\"",
                "",
                ItemInfoMessage::EditTargetFolder,
                ItemInfoMessage::BrowseTargetFolder,
            ),
            text(" "),
            text_input(
                "Change Notes (Optional)",
                &self.change_notes,
                ItemInfoMessage::EditChangeNotes
            ),
            text(" "),
            text("Once your mod is on the workshop, you can add screenshots and a description to it there."),
            text(" "),
            text("Don't touch the toggles below if you don't know what they mean!"),
            text(" "),
            checkbox("Use absolute path.", self.use_abs_path, ItemInfoMessage::AbsToggled),
            text(" "),
            checkbox("Convert rpy to rpym.", self.convert_rpy, ItemInfoMessage::ConvertToggled),
        ]
        .into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemInfo {
    pub name: String,
    pub preview_image: PathBuf,
    pub target_folder: PathBuf,
    pub change_notes: String,
    pub use_abs_path: bool,
    pub convert_rpy: bool,

}

impl From<ItemInfo> for ItemInfoState {
    fn from(value: ItemInfo) -> Self {
        ItemInfoState {
            name: value.name,
            preview_image: FileField::from(value.preview_image),
            target_folder: FileField::from(value.target_folder),
            change_notes: value.change_notes,
            use_abs_path: value.use_abs_path,
            convert_rpy: value.convert_rpy,
        }
    }
}

impl From<QueryResult> for ItemInfo {
    fn from(value: QueryResult) -> Self {
        ItemInfo {
            name: value.title,
            preview_image: PathBuf::new(),
            target_folder: PathBuf::new(),
            change_notes: String::new(),
            use_abs_path: false,
            convert_rpy: true,
        }
    }
}

impl TryFrom<ItemInfoState> for ItemInfo {
    type Error = String;

    fn try_from(value: ItemInfoState) -> Result<Self, Self::Error> {
        
        if value.name.is_empty() {
        return Err("Name cannot be empty.".to_string());
        }
        

        let preview_field_exists = value.preview_image.path.exists();
        let has_preview = preview_field_exists && value.preview_image.path.is_file();
        if !has_preview {
            if !value.preview_image.path.exists() {
                if value.preview_image.path.to_string_lossy().is_empty() {
                    return Err("Thumbnail image cannot be empty.".to_string());
                } else if !preview_field_exists {
                    return Err(format!(
                        "Thumbnail image \"{}\" does not exist.",
                        value.preview_image.path.to_string_lossy()
                    ));
                } else {
                    return Err(format!(
                        "Thumbnail image \"{}\" is not a file.",
                        value.preview_image.path.to_string_lossy()
                    ));
                }
            }
        }

        let modfolder_field_exists = value.target_folder.path.exists();
        let has_modfolder = modfolder_field_exists && value.target_folder.path.is_file();
        if !has_modfolder {
            if !value.target_folder.path.exists() {
                if value.target_folder.path.to_string_lossy().is_empty() {
                    return Err("Your mod's folder cannot be empty.".to_string());
                } else {
                return Err(format!(
                    "Mod folder \"{}\" does not exist.",
                    value.target_folder.path.to_string_lossy()
                ));
                }
            }
        }
        

        Ok(ItemInfo {
            name: value.name,
            preview_image: value.preview_image.path,
            target_folder: value.target_folder.path,
            change_notes: value.change_notes,
            use_abs_path: value.use_abs_path,
            convert_rpy: value.convert_rpy,
        })
    }
}
