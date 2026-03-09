use super::item_info::ItemInfo;
use crate::err_dialog_types::confirm_dialog;
use std::ops::Deref;
use std::sync::{atomic::AtomicUsize, atomic::Ordering, Arc};
use std::thread::Thread;
use std::time::Duration;
use steamworks::{Client, PublishedFileId, QueryResult, QueryResults, SingleClient, SteamError};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct SingleClientExecutor {
    watchers: Arc<AtomicUsize>,
    handle: Thread,
}

impl SingleClientExecutor {
    fn watch(&self) {
        self.watchers.fetch_add(1, Ordering::Release);
        self.handle.unpark()
    }

    fn unwatch(&self) {
        self.watchers.fetch_sub(1, Ordering::Acquire);
    }
}

fn start_executor(single_client: SingleClient) -> SingleClientExecutor {
    let watchers: Arc<AtomicUsize> = Arc::default();
    let thread_copy = watchers.clone();

    let handle = std::thread::Builder::new()
        .name("SingleClientExecutor".to_string())
        .spawn(move || steamworks_worker(single_client, thread_copy))
        .expect("Failed to start Steamworks thread.")
        .thread()
        .clone();

    SingleClientExecutor { watchers, handle }
}

fn steamworks_worker(single_client: SingleClient, mut watchers: Arc<AtomicUsize>) {
    loop {
        while watchers.load(Ordering::Acquire) > 0 {
            single_client.run_callbacks();
        }

        std::thread::park_timeout(Duration::from_millis(100));

        match Arc::try_unwrap(watchers) {
            Ok(_) => return,
            Err(arc) => watchers = arc,
        }
    }
}

#[derive(Debug)]
pub struct SingleClientExecutorWatcher {
    executor: SingleClientExecutor,
}

impl SingleClientExecutorWatcher {
    fn new(executor: SingleClientExecutor) -> Self {
        executor.watch();
        SingleClientExecutorWatcher { executor }
    }
}

impl Drop for SingleClientExecutorWatcher {
    fn drop(&mut self) {
        self.executor.unwatch();
    }
}

#[derive(Debug)]
pub struct CallbackSender<T> {
    _watcher: SingleClientExecutorWatcher,
    sender: iced::futures::channel::oneshot::Sender<T>,
}

impl<T> CallbackSender<T> {
    fn get_channel(
        executor: SingleClientExecutor,
    ) -> (Self, iced::futures::channel::oneshot::Receiver<T>) {
        let (tx, rx) = iced::futures::channel::oneshot::channel();
        let wtx = CallbackSender {
            _watcher: SingleClientExecutorWatcher::new(executor),
            sender: tx,
        };
        (wtx, rx)
    }

    fn send(self, value: T) -> Result<(), T> {
        self.sender.send(value)
    }
}

impl<T> Deref for CallbackSender<T> {
    type Target = iced::futures::channel::oneshot::Sender<T>;
    fn deref(&self) -> &Self::Target {
        &self.sender
    }
}

#[derive(Clone)]
pub struct WorkshopClient {
    callback_executor: SingleClientExecutor,
    steam_client: Client,
}

fn strip_verbatim_prefix(p: &std::path::Path) -> std::path::PathBuf {
    // Convert to string (Windows paths should be valid Unicode in your case)
    let s = p.to_string_lossy();

    // \\?\C:\...  => C:\...
    // \\?\UNC\server\share\... => \\server\share\...
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return std::path::PathBuf::from(format!(r"\\{}", rest));
    }
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        return std::path::PathBuf::from(rest);
    }

    p.to_path_buf()
}

impl WorkshopClient {
    pub fn init_app(id: steamworks::AppId) -> steamworks::SResult<Self> {
        Client::init_app(id).map(|(client, single_client)| WorkshopClient {
            callback_executor: start_executor(single_client),
            steam_client: client,
        })
    }

    pub fn open_url(&self, url: &str) -> () {
        // self.steam_client
        //     .friends()
        //     .activate_game_overlay_to_web_page(url)
        let _ = open::that(url);
    }

    pub fn open_terms(&self) -> () {
        const STEAM_LEGAL_AGREEMENT: &str =
            "https://steamcommunity.com/sharedfiles/workshoplegalagreement";

        self.open_url(STEAM_LEGAL_AGREEMENT)
    }

    pub async fn get_item_info(
        self: WorkshopClient,
        item_id: steamworks::PublishedFileId,
    ) -> Result<ItemInfo, SteamError> {
        let app_id = self.steam_client.utils().app_id();
        let (tx, rx) = CallbackSender::get_channel(self.callback_executor.clone());

        self.steam_client
            .ugc()
            .query_item(item_id)
            .expect("Failed to generate single item query.")
            .allow_cached_response(360)
            .include_long_desc(false)
            .include_children(false)
            .include_metadata(false)
            .include_additional_previews(false)
            .fetch(move |res| {
                let _ = tx.send(res.and_then(|res| res.get(0).ok_or(SteamError::NoMatch)));
            });
        rx.await
            .map_err(|iced::futures::channel::oneshot::Canceled| SteamError::Cancelled)
            .and_then(|x|x)
            .and_then(|res| match res.file_type {
                steamworks::FileType::Community => Ok(res),
                _ => Err(SteamError::NoMatch),
            })
            .and_then(|res| {
                if res.consumer_app_id != Some(app_id){
                    if confirm_dialog(format!("Found item\n\t\"{}\"\nappears to be for a different app than this uploader works with.\nYou may be blocked from uploading. Continue?",res.title).as_str()){
                        Ok(res)
                    }else{
                        Err(SteamError::Cancelled)
                    }
                } else {
                    Ok(res)
                }
            } )
            // .and_then(|res| {
            //         let user = self.steam_client.user().steam_id();
            //         if res.owner != user && !confirm_dialog("This Workshop entry appears to have been made by another user.\nYou may be blocked from uploading.\nContinue?"){
            //             // This check is, at present, not working.
            //             println!("\nOwner: {}\nUser: {}",res.owner.raw(), user.raw());
            //             Err(SteamError::AccessDenied)
            //         }else{
            //             Ok(res)
            //         }
            // })
            .map(Into::<ItemInfo>::into)
    }

    pub async fn create_item(self) -> Result<(PublishedFileId, bool), SteamError> {
        let app_id = self.steam_client.utils().app_id();
        let (tx, rx) = CallbackSender::get_channel(self.callback_executor.clone());

        self.steam_client
            .ugc()
            .create_item(app_id, steamworks::FileType::Community, move |res| {
                let _ = tx.send(res);
            });

        rx.await
            .map_err(|iced::futures::channel::oneshot::Canceled| SteamError::Cancelled)
            .and_then(|x| x)
    }

    pub async fn send_item(
        self,
        item_id: PublishedFileId,
        item_info: ItemInfo,
    ) -> Result<(PublishedFileId, bool), SteamError> {
        let temp_dir = std::env::temp_dir(); // Get the temporary directory
        let temp_folder = temp_dir.join("rivalhearts_workshop_folder"); // Create a specific folder within temporary directory
        let mut full_folder = temp_folder.join("mods");

        // Create the folders if they doesn't exist
        if !temp_folder.exists() {
            let _ = fs::create_dir(&temp_folder);
        }
        if !full_folder.exists() && !item_info.use_abs_path{
            let _ = fs::create_dir(&full_folder);
        }

        if item_info.use_abs_path {
            full_folder = temp_folder.clone();
        }

        println!("{}",item_info.convert_rpy);


        // // Copy the contents of the target folder to the temporary folder
        if !item_info.use_abs_path{
            let _ = fs_extra::dir::copy(&item_info.target_folder, &full_folder, &fs_extra::dir::CopyOptions::new());
        } else {
            let entries = match fs::read_dir(&item_info.target_folder) {
                Ok(entries) => entries,
                Err(_err) => return Err(SteamError::FileNotFound),
            };
            for entry in entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_err) => return Err(SteamError::FileNotFound),
                };

                let entry_path = entry.path();
                let relative_path = match entry_path.strip_prefix(Path::new(&item_info.target_folder)){
                    Ok(relative_path) => relative_path,
                    Err(_err) => return Err(SteamError::FileNotFound),
                };
                let destination_path = temp_folder.join(relative_path);


                if entry_path.is_file(){
                    let _ = fs::copy(entry_path, destination_path);
                } else if entry_path.is_dir() {
                    let _ = fs_extra::dir::copy(&entry_path, &temp_folder, &fs_extra::dir::CopyOptions::new());
                }
                
            }
        }

        
        if item_info.convert_rpy {
            let entries = WalkDir::new(&full_folder);
            for entry in entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_err) => return Err(SteamError::FileNotFound),
                };
                let path = entry.path();
                if let Some(extension) = path.extension() {
                    let extension_str = extension.to_string_lossy().to_lowercase();
                    if extension_str == "rpy" {
                        let new_path = path.with_extension("rpym");
                        if let Err(_err) = fs::rename(&path, &new_path) {
                            return Err(SteamError::FileNotFound);
                        }
                    } else if extension_str == "rpyc" {
                        if let Err(_err) = fs::remove_file(&path) {
                            return Err(SteamError::FileNotFound);
                        }
                    }
                }
            }
        }
        
        let rx = {
            let app_id = self.steam_client.utils().app_id();

            let change_notes = if item_info.change_notes.is_empty() {
                None
            } else {
                Some(item_info.change_notes.as_str())
            };

            let mut update_handle = self
                .steam_client
                .ugc()
                .start_item_update(app_id, item_id)
                .title(item_info.name.as_str())
                .content_path(&temp_folder); // Set the temporary folder as the content path

            if item_info.preview_image.exists() {

                update_handle = update_handle.preview_path(&item_info.preview_image)
            }

            let (tx, rx) = CallbackSender::get_channel(self.callback_executor.clone());

            let _update_watch_handle = update_handle.submit(change_notes, move |res| {
                let _ = tx.send(res);
            });

            rx
        };

        let result = rx.await
            .map_err(|iced::futures::channel::oneshot::Canceled| SteamError::Cancelled)
            .and_then(|x| x);

        // Clear the temporary folder after the upload is done
        let _ = std::fs::remove_dir_all(&full_folder);

        Ok(result?)
    }
}

fn _debug_query_result(result: QueryResult) {
    println!(
        "QueryResult: \"{}\" ({})",
        result.title, result.published_file_id.0
    );
    println!("Owner: {}", result.owner.raw());
    println!(
        "Description: {} words",
        result.description.split_whitespace().into_iter().count()
    );
    println!("File type: {:?}", result.file_type);
}

fn _debug_query_results(results: &QueryResults) {
    println!("QueryResults: (FromCache: {})", results.was_cached());
    let result_count = results.total_results();
    for (i, result) in results.iter().enumerate() {
        if let Some(result) = result {
            println!("Result {}/{}", i, result_count);
            _debug_query_result(result);
        } else {
            println!("Result #{}: None", i);
        }
    }
}
