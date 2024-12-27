use std::time::Duration;

use proc_heim::{Cmd, CmdOptionsBuilder, GetLogsError, LoggingType, LogsQuery};
use test_utils::cmd_collection::{
    bash_script, echo_cmd_with_options,
    std_io::{echo_all_args_script_path, echo_stderr_script},
};

use crate::common::create_process_manager;

mod common;
mod test_cases;

#[tokio::test]
async fn should_read_logs_from_stdout() {
    check_logs_from_stdout(LoggingType::StdoutOnly, true).await;
    check_logs_from_stdout(LoggingType::StdoutAndStderr, true).await;
    check_logs_from_stdout(LoggingType::StdoutAndStderrMerged, true).await;
    check_logs_from_stdout(LoggingType::StderrOnly, false).await;
}

#[tokio::test]
async fn should_read_logs_from_stderr() {
    check_logs_from_stderr(LoggingType::StdoutOnly, false).await;
    check_logs_from_stderr(LoggingType::StdoutAndStderr, true).await;
    check_logs_from_stderr(LoggingType::StdoutAndStderrMerged, true).await;
    check_logs_from_stderr(LoggingType::StderrOnly, true).await;
}

async fn check_logs_from_stdout(logging_type: LoggingType, should_logs_be_set: bool) {
    let (_dir, handle) = create_process_manager();
    let log = "just an example log data";
    let cmd = echo_cmd_with_logging(log, logging_type);
    let process_id = handle.spawn(cmd).await.unwrap();
    let _ = handle
        .wait(process_id, Duration::from_millis(100))
        .await
        .await;

    let query = LogsQuery::default();
    let result = handle.get_logs_stdout(process_id, query).await;
    assert_logs(result, log, should_logs_be_set);
}

async fn check_logs_from_stderr(logging_type: LoggingType, should_logs_be_set: bool) {
    let (_dir, handle) = create_process_manager();
    let log = "funny error log data";
    let cmd = echo_to_stderr_cmd_with_logging(log, logging_type);
    let process_id = handle.spawn(cmd).await.unwrap();

    let _ = handle
        .wait(process_id, Duration::from_millis(100))
        .await
        .await;

    let query = LogsQuery::default();
    let result = handle.get_logs_stderr(process_id, query).await;
    assert_logs(result, log, should_logs_be_set);
}

fn assert_logs(
    result: Result<Vec<String>, GetLogsError>,
    expected_log: &str,
    should_logs_be_set: bool,
) {
    if should_logs_be_set {
        let logs = result.unwrap();
        assert_eq!(1, logs.len());
        assert_eq!(expected_log, logs.first().unwrap());
    } else {
        assert!(result.is_err());

        assert!(matches!(
            result.err().unwrap(),
            GetLogsError::LoggingTypeWasNotConfigured(_)
        ))
    }
}

fn echo_cmd_with_logging(msg: &str, logging_type: LoggingType) -> Cmd {
    let options = CmdOptionsBuilder::default()
        .logging_type(logging_type)
        .build()
        .unwrap();
    echo_cmd_with_options(msg, options)
}

fn echo_to_stderr_cmd_with_logging(msg: &str, logging_type: LoggingType) -> Cmd {
    let options = CmdOptionsBuilder::default()
        .logging_type(logging_type)
        .build()
        .unwrap();
    bash_script(echo_stderr_script(), options, vec![msg.into()])
}

#[tokio::test]
async fn should_query_logs_with_offset_and_limit() {
    let (_dir, handle) = create_process_manager();
    let expected_logs = generate_logs();
    let cmd = echo_all_args_script(&expected_logs);
    let process_id = handle.spawn(cmd).await.unwrap();

    let _ = handle
        .wait(process_id, Duration::from_millis(100))
        .await
        .await;

    let query = LogsQuery::new(None, Some(2));
    let logs = handle.get_logs_stdout(process_id, query).await.unwrap();
    assert_eq!(expected_logs[0..2], logs);

    let query = LogsQuery::new(Some(3), None);
    let logs = handle.get_logs_stdout(process_id, query).await.unwrap();
    assert_eq!(expected_logs[3..10], logs);

    let query = LogsQuery::new(Some(0), Some(2));
    let logs = handle.get_logs_stdout(process_id, query).await.unwrap();
    assert_eq!(expected_logs[0..2], logs);

    let query = LogsQuery::new(Some(1), Some(2));
    let logs = handle.get_logs_stdout(process_id, query).await.unwrap();
    assert_eq!(expected_logs[1..3], logs);

    let query = LogsQuery::new(Some(5), Some(4));
    let logs = handle.get_logs_stdout(process_id, query).await.unwrap();
    assert_eq!(expected_logs[5..9], logs);

    let query = LogsQuery::new(Some(5), Some(20));
    let logs = handle.get_logs_stdout(process_id, query).await.unwrap();
    assert_eq!(expected_logs[5..10], logs);

    let query = LogsQuery::new(Some(9), Some(20));
    let logs = handle.get_logs_stdout(process_id, query).await.unwrap();
    assert_eq!(expected_logs[9..10], logs);

    let query = LogsQuery::new(Some(10), Some(1));
    let logs = handle.get_logs_stdout(process_id, query).await.unwrap();
    assert!(logs.is_empty());

    let query = LogsQuery::new(Some(11), Some(1));
    let logs = handle.get_logs_stdout(process_id, query).await.unwrap();
    assert!(logs.is_empty());

    let query = LogsQuery::new(Some(11), Some(0));
    let logs = handle.get_logs_stdout(process_id, query).await.unwrap();
    assert!(logs.is_empty());

    let query = LogsQuery::new(Some(2), Some(0));
    let logs = handle.get_logs_stdout(process_id, query).await.unwrap();
    assert!(logs.is_empty());
}

fn generate_logs() -> Vec<String> {
    let mut logs = Vec::new();
    for i in 0..10 {
        logs.push(format!("log no. {i}"));
    }
    logs
}

fn echo_all_args_script(args: &[String]) -> Cmd {
    let options = CmdOptionsBuilder::default()
        .logging_type(LoggingType::StdoutAndStderr)
        .build()
        .unwrap();
    bash_script(echo_all_args_script_path(), options, args.to_vec())
}
