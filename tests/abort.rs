use proc_heim::manager::WriteMessageError;
use test_utils::cmd_collection::std_io::echo_cmd;

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
