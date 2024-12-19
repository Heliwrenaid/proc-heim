use std::{fmt::Debug, path::PathBuf};

use crate::Cmd;

pub trait Runnable: Debug + Send + 'static {
    fn bootstrap_cmd(&self, process_dir: &PathBuf) -> Result<Cmd, String>;
    #[allow(unused_variables)]
    fn clean_after_fail(&self, process_dir: &PathBuf) -> Result<(), String> {
        Ok(()) // by default ProcessSpawner deletes whole process_dir on fail. This method is meant for extra cleanup.
    }
}
