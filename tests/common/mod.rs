use proc_heim::{ProcessManager, ProcessManagerHandle};
use tempfile::{tempdir, TempDir};

pub fn create_process_manager() -> (TempDir, ProcessManagerHandle) {
    let working_dir = tempdir().unwrap();
    let handle = ProcessManager::new(working_dir.path().into());
    (working_dir, handle)
}
