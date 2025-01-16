use proc_heim::manager::{ProcessManager, ProcessManagerHandle};
use tempfile::{tempdir, TempDir};

pub fn create_process_manager() -> (TempDir, ProcessManagerHandle) {
    let working_dir = tempdir().unwrap();
    let handle = ProcessManager::spawn(working_dir.path().into()).unwrap();
    (working_dir, handle)
}
