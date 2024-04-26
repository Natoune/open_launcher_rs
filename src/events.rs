use tokio::sync::broadcast;

use crate::Launcher;

/// Progress event.
/// The task is the name of the task that is currently running.
/// It can be one of the following:
/// - `checking_assets`: Checking if assets are up to date.
/// - `downloading_assets`: Downloading missing assets.
/// - `checking_libraries`: Checking if libraries are up to date.
/// - `downloading_libraries`: Downloading missing libraries.
/// - `checking_natives`: Checking if natives are up to date.
/// - `extracting_natives`: Extracting natives.
/// - `post_processing`: Post-processing Forge (or NeoForge).
/// The file is the name of the file or the library that is currently being processed.
/// The total is the total number of bytes/elements to process for the current task.
/// The current is the number of bytes/elements that have been processed for the current task.
#[derive(Clone)]
pub struct Progress {
    pub task: String,
    pub file: String,
    pub total: u64,
    pub current: u64,
}

impl Launcher {
    /// Returns a receiver for progress events.
    pub fn on_progress(&self) -> broadcast::Receiver<Progress> {
        self.progress_receiver.resubscribe()
    }

    pub(crate) fn emit_progress(&mut self, task: &str, file: &str, total: u64, current: u64) {
        self.progress = Progress {
            task: task.to_string(),
            file: file.to_string(),
            total,
            current,
        };
        let _ = self.progress_sender.send(self.progress.clone());
    }
}
