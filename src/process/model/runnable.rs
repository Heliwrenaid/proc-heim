use std::{fmt::Debug, path::Path};

use super::Cmd;

pub trait Runnable: Debug + Send + 'static {
    fn bootstrap_cmd(&self, process_dir: &Path) -> Result<Cmd, String>;
    #[allow(unused_variables)]
    fn clean_after_fail(&self, process_dir: &Path) -> Result<(), String> {
        Ok(()) // by default ProcessSpawner deletes whole process_dir on fail. This method is meant for extra cleanup.
    }
}
