use std::path::PathBuf;

use proc_heim::{ProcessManager, ProcessManagerHandle};
use tempfile::{tempdir, TempDir};

pub fn create_process_manager() -> (TempDir, ProcessManagerHandle) {
    let working_dir = tempdir().unwrap();
    let handle = ProcessManager::new(working_dir.path().into());
    (working_dir, handle)
}

pub fn echo_script() -> String {
    scripts_dir().join("echo.sh").to_str().unwrap().into()
}

fn scripts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts")
}
