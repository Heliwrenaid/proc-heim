use std::time::Duration;

use crate::common::create_process_manager;

use futures::{FutureExt, StreamExt, TryStreamExt};
use proc_heim::{
    manager::{GetProcessInfoError, LogsQuery, TryMessageStreamExt},
    model::script::ScriptingLanguage,
};
use test_utils::{cmd_collection::std_io::echo_cmd, scripts_collection::*};

mod common;
mod test_cases;

#[cfg(feature = "json")]
use crate::test_cases::ExampleMessage;
#[cfg(feature = "json")]
use proc_heim::model::command::CmdOptions;

#[tokio::test]
async fn test_process_handle_wrapper() {
    let (_dir, manager_handle) = create_process_manager();

    let arg: &str = "example argument with spaces";
    let args = vec![arg.to_owned()];
    let message_to_sent = "Test message";

    let script = build_echo_script(ScriptingLanguage::Bash, BASH_ECHO_SCRIPT, &args);

    let handle = manager_handle
        .spawn_with_handle(script.clone())
        .await
        .unwrap();

    handle.send_message(message_to_sent).await.unwrap();

    let mut stream = handle
        .subscribe_message_stream()
        .await
        .unwrap()
        .into_string_stream();
    let message = stream.try_next().await.unwrap().unwrap();
    assert_eq!(message_to_sent, message);
    assert!(stream.next().now_or_never().is_none());

    let stdout = handle
        .get_logs_stdout(LogsQuery::fetch_all())
        .await
        .unwrap();

    assert_eq!(2, stdout.len());
    assert_eq!(format!("First Argument: {arg}"), *stdout.first().unwrap());
    assert_eq!(
        format!("Received: {message_to_sent}"),
        *stdout.get(1).unwrap()
    );

    let errors = handle
        .get_logs_stderr(LogsQuery::fetch_all())
        .await
        .unwrap();

    assert_eq!(1, errors.len());
    assert_eq!(
        format!("Error: {message_to_sent}"),
        *errors.first().unwrap()
    );

    let process_data = handle
        .wait(Duration::from_millis(500))
        .await
        .unwrap()
        .unwrap();
    assert!(process_data.exit_status().unwrap().success());
}

#[tokio::test]
async fn should_kill_process() {
    let (dir, manager_handle) = create_process_manager();
    let script = build_echo_script(
        ScriptingLanguage::Bash,
        BASH_ECHO_SCRIPT,
        &["arg".to_owned()],
    );
    let handle = manager_handle.spawn_with_handle(script).await.unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;

    let process_data = handle.get_process_info().await.unwrap();
    assert!(process_data.is_running());

    let process_dir = dir.path().join(handle.id().to_string());

    assert!(process_dir.exists());
    assert!(handle.kill().await.is_ok());
    assert!(!process_dir.exists());

    let result = handle.get_process_info().await;
    assert!(matches!(
        result,
        Err(GetProcessInfoError::ProcessNotFound(_))
    ));
}

#[cfg(feature = "json")]
#[tokio::test]
async fn should_write_and_read_json() {
    use proc_heim::manager::serde::MessageFormat;
    use proc_heim::manager::{Message, ResultStreamExt};
    use proc_heim::model::script::Script;

    let (_dir, manager_handle) = create_process_manager();
    let script = Script::with_options(
        ScriptingLanguage::Bash,
        r#"
            read msg < /dev/stdin
            echo "$msg"
            "#,
        CmdOptions::with_standard_io_messaging(),
    );
    let handle = manager_handle.spawn_with_handle(script).await.unwrap();

    let data1 = "Some message";
    let example_message = ExampleMessage {
        data1: data1.into(),
        ..Default::default()
    };

    let message = Message::from_serializable(&example_message, &MessageFormat::Json).unwrap();
    handle.send_message(message).await.unwrap();

    let mut stream = handle
        .subscribe_message_stream()
        .await
        .unwrap()
        .into_deserialized_stream::<ExampleMessage>(&MessageFormat::Json)
        .filter_ok();
    let next_message = stream.next().await.unwrap();
    assert_eq!(next_message, example_message);
}

#[tokio::test]
async fn should_wait_for_process_completion() {
    let (_dir, manager_handle) = create_process_manager();
    let handle = manager_handle
        .spawn_with_handle(echo_cmd("some message"))
        .await
        .unwrap();

    let data = handle
        .wait(Duration::from_micros(100))
        .await
        .unwrap()
        .unwrap();
    assert!(data.exit_status().is_some());
}
