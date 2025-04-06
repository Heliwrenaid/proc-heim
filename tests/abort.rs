use std::{fs, time::Duration};

use proc_heim::manager::WriteMessageError;
use sysinfo::{Pid, System};
use tempfile::TempDir;
use test_utils::cmd_collection::{hanging_forever_cmd, std_io::echo_cmd};

use crate::common::create_process_manager;

mod common;

#[tokio::test]
async fn should_return_proper_error_after_aborting_manager_task() {
    let (_dir, manager) = create_process_manager();
    let id = manager.spawn(echo_cmd("msg")).await.unwrap();

    let result = manager.abort().await;
    assert!(result.is_ok());

    let result = manager.send_message(id, "msg").await;
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(matches!(
        err,
        WriteMessageError::ManagerCommunicationError(_)
    ))
}

#[tokio::test]
async fn should_return_error_after_aborting_manager_two_times() {
    let (_dir, manager) = create_process_manager();
    manager.spawn(echo_cmd("msg")).await.unwrap();

    let result = manager.abort().await;
    assert!(result.is_ok());

    let result = manager.abort().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn should_clear_working_dir_on_abort() {
    let (dir, manager) = create_process_manager();
    assert!(is_dir_empty(&dir));
    manager.spawn(echo_cmd("msg")).await.unwrap();

    assert!(!is_dir_empty(&dir));

    let result = manager.abort().await;
    assert!(result.is_ok());
    assert!(is_dir_empty(&dir));
}

#[tokio::test]
async fn should_clear_working_dir_on_drop() {
    let (dir, manager) = create_process_manager();
    assert!(is_dir_empty(&dir));
    manager.spawn(echo_cmd("msg")).await.unwrap();

    assert!(!is_dir_empty(&dir));

    drop(manager);
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(is_dir_empty(&dir));
}

#[tokio::test]
async fn should_preserve_not_own_files_in_working_dir_on_drop() {
    let (dir, manager) = create_process_manager();
    assert!(is_dir_empty(&dir));
    let filename = "file.txt";
    fs::write(dir.path().join(filename), "abc").unwrap();
    manager.spawn(echo_cmd("msg")).await.unwrap();

    drop(manager);
    tokio::time::sleep(Duration::from_secs(1)).await;

    let files: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(1, files.len());
    let actual_filename = files[0]
        .as_ref()
        .unwrap()
        .file_name()
        .into_string()
        .unwrap();
    assert_eq!(filename, actual_filename);
}

#[tokio::test]
async fn should_kill_processes_on_drop() {
    let (_dir, manager) = create_process_manager();

    let mut pid_list = vec![];
    for _ in 0..100 {
        let id = manager.spawn(hanging_forever_cmd()).await.unwrap();
        let info = manager.get_process_info(id).await.unwrap();
        pid_list.push(info.pid().unwrap());
    }
    assert_processes(&pid_list, true);

    drop(manager);
    tokio::time::sleep(Duration::from_secs(1)).await;

    assert_processes(&pid_list, false);
}

#[tokio::test]
async fn should_not_abort_manager_when_cloned_handle_is_dropped() {
    let (_dir, manager) = create_process_manager();
    let cloned_handle = manager.clone();

    drop(cloned_handle);
    tokio::time::sleep(Duration::from_secs(1)).await;

    let result = manager.spawn(echo_cmd("msg")).await;
    assert!(result.is_ok());
}

fn assert_processes(pid_list: &[u32], is_running: bool) {
    let sys = System::new_all();
    for pid in pid_list {
        assert!(sys.process(Pid::from_u32(*pid)).is_some() == is_running);
    }
}

pub fn is_dir_empty(dir: &TempDir) -> bool {
    dir.path().read_dir().unwrap().next().is_none()
}
