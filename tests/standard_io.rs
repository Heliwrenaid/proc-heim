use proc_heim::model::command::{Cmd, CmdOptions};
use proptest::collection::vec;
use proptest::prelude::*;
use test_utils::cmd_collection::{
    bash_script, hanging_forever_cmd,
    std_io::{self, echo_cmd},
};

use crate::{common::create_process_manager, test_cases::ExampleMessage};
mod common;
mod test_cases;

#[tokio::test]
pub async fn should_spawn_process() {
    let (_dir, handle) = create_process_manager();
    let result = handle.spawn(hanging_forever_cmd()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn should_read_message() {
    test_cases::should_read_message(echo_cmd).await;
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

    #[cfg(all(feature = "json", feature = "message-pack"))]
    #[test]
    fn should_read_message_with_different_formats(
            data1 in "[a-zA-Z0-9]{1,100}",
            data2 in vec(any::<u8>(), 0..100),
            data3 in any::<i32>(),
            data4 in any::<f32>(),
            data5 in any::<bool>()
    ) {
        use proc_heim::model::serde::{DataFormat};

        #[cfg(feature = "message-pack")]
        use proc_heim::model::serde::{Encoding};

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
                test_cases::should_read_message_with_format(cmd.clone(), message.clone(), DataFormat::Json).await;
                test_cases::should_read_message_with_format(cmd.clone(), message.clone(), DataFormat::MessagePack(Encoding::Hex)).await;
                test_cases::should_read_message_with_format(cmd.clone(), message, DataFormat::MessagePack(Encoding::Base64)).await;
            });
    }
}

#[tokio::test]
async fn should_write_json_message_and_read_part_of_it() {
    test_cases::should_write_json_message_and_read_part_of_it(echo_json_script).await;
}

fn echo_daemon_script() -> Cmd {
    bash_script(
        std_io::echo_daemon_script_path(),
        CmdOptions::with_standard_io_messaging(),
        vec![],
    )
}

fn echo_json_script(json_path: &str) -> Cmd {
    bash_script(
        std_io::echo_json_script(),
        CmdOptions::with_standard_io_messaging(),
        vec![json_path.into()],
    )
}
