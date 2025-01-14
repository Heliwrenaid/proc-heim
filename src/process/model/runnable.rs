use std::{fmt::Debug, path::Path};

use super::Cmd;

/// Trait, which enables using of user-defined types for creating a custom process.
pub trait Runnable: Debug + Send + 'static {
    /// This method should prepare process to run and return [`Cmd`](struct@crate::model::command::Cmd) used to spawn a custom process.
    /// If you need to create some files for properly spawning a process, do it inside provided `process_dir` directory.
    fn bootstrap_cmd(&self, process_dir: &Path) -> Result<Cmd, String>;
    /// This method is called when process spawning fails.
    /// Notice that `process_dir` will be deleted automatically, so there is no need to delete it here.
    #[allow(unused_variables)]
    fn clean_after_fail(&self, process_dir: &Path) -> Result<(), String> {
        Ok(())
    }
}
