use std::time::Duration;

use crate::common::create_process_manager;
use futures::FutureExt;
use proc_heim::{GetProcessDataError, LogsQuery, ScriptLanguage};
use test_utils::{cmd_collection::std_io::echo_cmd, scripts_collection::*};
use tokio_stream::StreamExt;

mod common;
mod test_cases;

#[cfg(feature = "json")]
use crate::test_cases::ExampleMessage;
#[cfg(feature = "json")]
use proc_heim::CmdOptions;

#[tokio::test]
async fn test_process_handle_wrapper() {
    let (_dir, manager_handle) = create_process_manager();

    let arg: &str = "example argument with spaces";
    let args = vec![arg.to_owned()];
    let message_to_sent = "Test message";

    let script = build_echo_script(ScriptLanguage::Bash, BASH_ECHO_SCRIPT, &args);

    let handle = manager_handle
        .spawn_with_handle(script.clone())
        .await
        .unwrap();

    handle.write_message(message_to_sent).await.unwrap();

    let mut stream = handle.subscribe_message_string_stream().await.unwrap();
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
    let (_dir, manager_handle) = create_process_manager();
    let script = build_echo_script(ScriptLanguage::Bash, BASH_ECHO_SCRIPT, &["arg".to_owned()]);
    let handle = manager_handle.spawn_with_handle(script).await.unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;

    let process_data = handle.get_process_data().await.unwrap();
    assert!(process_data.is_running());

    assert!(handle.kill().await.is_ok());

    let result = handle.get_process_data().await;
    assert!(matches!(
        result,
        Err(GetProcessDataError::ProcessNotFound(_))
    ));
}

#[cfg(feature = "json")]
#[tokio::test]
async fn should_write_and_read_json() {
    use proc_heim::{DataFormat, Script};

    let (_dir, manager_handle) = create_process_manager();
    let script = Script::with_options(
        ScriptLanguage::Bash,
        r#"
            read msg < /dev/stdin
            echo "$msg"
            "#,
        CmdOptions::standard_io(),
    );
    let handle = manager_handle.spawn_with_handle(script).await.unwrap();

    let data1 = "Some message";
    let message = ExampleMessage {
        data1: data1.into(),
        ..Default::default()
    };

    handle
        .write_messages_with_format(&message, DataFormat::Json)
        .await
        .unwrap();

    let mut stream = handle
        .subscribe_message_stream_with_format(DataFormat::Json)
        .await
        .unwrap();
    let next_message: ExampleMessage = stream.try_next().await.unwrap().unwrap();
    assert_eq!(next_message, message);
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
