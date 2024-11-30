use proc_heim::CmdOptions;
use test_utils::cmd_collection::{
    bash_script, cat_cmd,
    std_io::{self, echo_cmd},
};

use crate::common::create_process_manager;
mod common;
mod test_cases;

#[tokio::test]
pub async fn should_spawn_process() {
    let (_dir, handle) = create_process_manager();
    let result = handle.spawn(cat_cmd()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn should_read_message() {
    test_cases::should_read_message(echo_cmd).await;
}

#[tokio::test]
async fn should_spawn_process_then_communicate_with_it_then_kill() {
    let cmd = bash_script(
        std_io::echo_daemon_script_path(),
        CmdOptions::std_io(),
        vec![],
    );
    test_cases::should_spawn_process_then_communicate_with_it_then_kill(cmd).await;
}
