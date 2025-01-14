use std::time::Duration;

use sysinfo::{Pid, System};
use test_utils::cmd_collection::{hanging_forever_cmd, std_io::echo_cmd};

use crate::common::create_process_manager;

mod common;

#[tokio::test]
async fn should_get_data_of_not_completed_process() {
    let (_dir, handle) = create_process_manager();
    let process_id = handle.spawn(hanging_forever_cmd()).await.unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;

    let process_data = handle.get_process_info(process_id).await.unwrap();
    dbg!(&process_data);
    assert!(process_data.is_running());
    assert!(process_data.exit_status().is_none());
    assert!(process_data.pid().is_some());
}

#[tokio::test]
async fn should_get_data_of_completed_process() {
    let (_dir, handle) = create_process_manager();
    let process_id = handle.spawn(echo_cmd("msg")).await.unwrap();

    tokio::time::sleep(Duration::from_secs(1)).await;

    let process_data = handle.get_process_info(process_id).await.unwrap();

    assert!(!process_data.is_running());
    assert!(process_data.exit_status().unwrap().success());
    assert!(process_data.pid().is_some());
}

#[tokio::test]
async fn should_release_pid_after_process_completion() {
    let (_dir, handle) = create_process_manager();
    let process_id = handle.spawn(echo_cmd("msg")).await.unwrap();

    tokio::time::sleep(Duration::from_secs(1)).await;

    let process_data = handle.get_process_info(process_id).await.unwrap();
    assert!(!process_data.is_running());
    let pid = process_data.pid().unwrap();
    let pid = Pid::from_u32(pid);
    let sys = System::new_all();
    assert!(sys.process(pid).is_none());
}
