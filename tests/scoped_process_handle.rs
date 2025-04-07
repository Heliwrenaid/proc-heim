use std::time::Duration;

use common::create_process_manager;
use proc_heim::manager::{GetProcessInfoError, ScopedProcessHandle};
use sysinfo::{Pid, System};
use test_utils::cmd_collection::hanging_forever_cmd;

mod common;

#[tokio::test]
async fn should_kill_process_on_drop() {
    let (_dir, manager_handle) = create_process_manager();

    let handle = manager_handle
        .spawn_with_scoped_handle(hanging_forever_cmd())
        .await
        .unwrap();
    let info = handle.get_process_info().await.unwrap();
    assert!(info.pid().is_some());
    let pid = info.pid().unwrap();
    let process_id = *handle.id();
    drop(handle);
    tokio::time::sleep(Duration::from_secs(1)).await;

    let sys = System::new_all();
    assert!(sys.process(Pid::from_u32(pid)).is_none());

    let result = manager_handle.get_process_info(process_id).await;
    assert!(matches!(
        result,
        Err(GetProcessInfoError::ProcessNotFound(_))
    ));
}

#[tokio::test]
async fn should_not_kill_process_while_dropping_cloned_handle() {
    let (_dir, manager_handle) = create_process_manager();

    let handle = manager_handle
        .spawn_with_scoped_handle(hanging_forever_cmd())
        .await
        .unwrap();
    let process_id = *handle.id();
    let cloned_handle = handle.clone();

    // Act 1 - drop cloned handle, should not kill the process
    drop(cloned_handle);

    tokio::time::sleep(Duration::from_secs(1)).await;

    let info = manager_handle.get_process_info(process_id).await.unwrap();
    assert!(info.is_running());

    // Act 2 - drop original handle, should kill the process
    drop(handle);
    tokio::time::sleep(Duration::from_secs(1)).await;

    let result = manager_handle.get_process_info(process_id).await;
    assert!(matches!(
        result,
        Err(GetProcessInfoError::ProcessNotFound(_))
    ));
}

#[tokio::test]
async fn should_kill_process_when_converted_to_scoped_process_handle() {
    let (_dir, manager_handle) = create_process_manager();

    let handle = manager_handle
        .spawn_with_handle(hanging_forever_cmd())
        .await
        .unwrap();
    let scoped_handle: ScopedProcessHandle = handle.into();
    let process_id = *scoped_handle.id();

    let info = manager_handle.get_process_info(process_id).await.unwrap();
    assert!(info.is_running());

    drop(scoped_handle);

    let result = manager_handle.get_process_info(process_id).await;
    assert!(matches!(
        result,
        Err(GetProcessInfoError::ProcessNotFound(_))
    ));
}
