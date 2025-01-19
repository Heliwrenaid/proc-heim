use proc_heim::model::command::{Cmd, CmdOptions};
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
    #[cfg(all(feature = "json", feature = "message-pack"))]
    #[test]
    fn should_read_message_with_different_formats(
            data1 in "[a-zA-Z0-9]{1,100}",
            data2 in vec(any::<u8>(), 0..100),
            data3 in any::<i32>(),
            data4 in any::<f32>(),
            data5 in any::<bool>()
    ) {
        use proc_heim::manager::serde::MessageFormat;

        #[cfg(feature = "message-pack")]
        use proc_heim::manager::serde::Encoding;

        let cmd = echo_daemon_script();
        let message = ExampleMessage {
            data1,
            data2,
            data3,
            data4,
            data5,
        };
        tokio::runtime::Runtime::new().unwrap()
            .block_on(async {
                test_cases::should_read_message_with_format(cmd.clone(), message.clone(), MessageFormat::Json).await;
                test_cases::should_read_message_with_format(cmd.clone(), message.clone(), MessageFormat::MessagePack(Encoding::Hex)).await;
                test_cases::should_read_message_with_format(cmd.clone(), message, MessageFormat::MessagePack(Encoding::Base64)).await;
            });
    }
}

#[cfg(any(feature = "json", feature = "message-pack"))]
#[tokio::test]
async fn should_write_json_message_and_read_part_of_it() {
    test_cases::should_write_json_message_and_read_part_of_it(echo_json_script).await;
}

fn echo_daemon_script() -> Cmd {
    bash_script(
        named_pipe::echo_daemon_script_path(),
        CmdOptions::with_named_pipe_messaging(),
        vec![],
    )
}

fn echo_json_script(json_path: &str) -> Cmd {
    bash_script(
        named_pipe::echo_json_script(),
        CmdOptions::with_named_pipe_messaging(),
        vec![json_path.into()],
    )
}
