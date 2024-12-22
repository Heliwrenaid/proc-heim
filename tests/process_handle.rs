use std::time::Duration;

use crate::{common::create_process_manager, test_cases::ExampleMessage};
use futures::FutureExt;
use proc_heim::{CmdOptions, GetProcessDataError, LogsQuery, ScriptBuilder, ScriptLanguage};
use test_utils::scripts_collection::*;
use tokio_stream::StreamExt;

mod common;
mod test_cases;

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

    tokio::time::sleep(Duration::from_secs(1)).await;

    handle.write_message(message_to_sent).await.unwrap();

    let mut stream = handle.subscribe_message_bytes_stream().await.unwrap();
    let message = stream.try_next().await.unwrap().unwrap();
    assert_eq!(message_to_sent.as_bytes(), message);
    assert!(stream.next().now_or_never().is_none());

    let stdout = handle.get_logs_stdout(LogsQuery::default()).await.unwrap();

    assert_eq!(2, stdout.len());
    assert_eq!(format!("First Argument: {arg}"), *stdout.get(0).unwrap());
    assert_eq!(
        format!("Received: {message_to_sent}"),
        *stdout.get(1).unwrap()
    );

    let errors = handle.get_logs_stderr(LogsQuery::default()).await.unwrap();

    assert_eq!(1, errors.len());
    assert_eq!(format!("Error: {message_to_sent}"), *errors.get(0).unwrap());

    tokio::time::sleep(Duration::from_secs(1)).await;

    let process_data = handle.get_process_data().await.unwrap();
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
    use proc_heim::DataFormat;

    let (_dir, manager_handle) = create_process_manager();
    let script = ScriptBuilder::default()
        .lang(ScriptLanguage::Bash)
        .content(
            r#"
            read msg < /dev/stdin
            echo "$msg"
            "#,
        )
        .options(CmdOptions::std_io())
        .build()
        .unwrap();
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
