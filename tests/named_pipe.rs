use proc_heim::CmdOptions;
use test_utils::cmd_collection::{bash_script, named_pipe};
mod common;
mod test_cases;

#[tokio::test]
async fn should_read_message() {
    test_cases::should_read_message(named_pipe::echo_script).await;
}

#[tokio::test]
async fn should_spawn_process_then_communicate_with_it_then_kill() {
    let cmd = bash_script(
        named_pipe::echo_daemon_script_path(),
        CmdOptions::named_pipe(),
        vec![],
    );
    test_cases::should_spawn_process_then_communicate_with_it_then_kill(cmd).await;
}
