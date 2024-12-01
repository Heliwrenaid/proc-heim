use proc_heim::{Cmd, CmdOptions};
use proptest::collection::vec;
use proptest::prelude::*;
use test_utils::cmd_collection::{bash_script, named_pipe};

use crate::test_cases::ExampleMessage;
mod common;
mod test_cases;

#[tokio::test]
async fn should_read_message() {
    test_cases::should_read_message(named_pipe::echo_script).await;
}

#[tokio::test]
async fn should_spawn_process_then_communicate_with_it_then_kill() {
    let cmd = echo_daemon_script();
    test_cases::should_spawn_process_then_communicate_with_it_then_kill(cmd).await;
}

proptest! {
    #[test]
    fn should_read_structured_message(
            data1 in "[a-zA-Z0-9]{1,100}",
            data2 in vec(any::<u8>(), 0..100),
            data3 in any::<i32>(),
            data4 in any::<f32>(),
            data5 in any::<bool>()
    ) {
        let cmd = echo_daemon_script();
        let message = ExampleMessage {
            data1,
            data2,
            data3,
            data4,
            data5,
        };

        tokio::runtime::Runtime::new().unwrap()
            .block_on(test_cases::should_read_structured_message(cmd.clone(), message));
    }
}

#[tokio::test]
async fn should_write_json_message_and_read_part_of_it() {
    test_cases::should_write_json_message_and_read_part_of_it(echo_json_script).await;
}

fn echo_daemon_script() -> Cmd {
    bash_script(
        named_pipe::echo_daemon_script_path(),
        CmdOptions::named_pipe(),
        vec![],
    )
}

fn echo_json_script(json_path: &str) -> Cmd {
    bash_script(
        named_pipe::echo_json_script(),
        CmdOptions::named_pipe(),
        vec![json_path.into()],
    )
}
