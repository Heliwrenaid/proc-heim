use std::{env, time::Duration};

use proc_heim::{Cmd, CmdOptions, LoggingType, LogsQuery, Runnable};

use crate::common::create_process_manager;

mod common;

#[tokio::test]
async fn should_get_inherited_variable() {
    let env_name = "TEST_VAR_1";
    let env_value = "just a value";

    env::set_var(env_name, env_value);

    let cmd = get_env_cmd(env_name);
    let actual_env_value = spawn_and_read_message(cmd).await;
    assert_eq!(env_value, actual_env_value.unwrap());
}

#[tokio::test]
async fn should_set_environment_variable() {
    let env_name = "TEST_VAR";
    let env_value = "some text";

    let mut cmd = get_env_cmd(env_name);
    cmd.options_mut().add_env(env_name, env_value);
    let actual_env_value = spawn_and_read_message(cmd).await;

    assert_eq!(env_value, actual_env_value.unwrap());
}

#[tokio::test]
async fn should_clear_all_env_vars() {
    let mut cmd = Cmd::new("env");
    let mut options = CmdOptions::with_logging(LoggingType::StdoutOnly);
    options.set_clear_envs(true);
    cmd.set_options(options);

    let actual_env_value = spawn_and_read_message(cmd).await;

    assert!(actual_env_value.is_none());
}

#[tokio::test]
async fn should_remove_environment_variable() {
    let env_name = "TEST_VAR";
    let env_value = "Hey";

    env::set_var(env_name, env_value);

    let mut cmd = get_env_cmd(env_name);
    cmd.options_mut().remove_env(env_name);
    let actual_env_value = spawn_and_read_message(cmd).await;

    assert!(actual_env_value.is_none());
}

#[tokio::test]
async fn should_update_environment_variable() {
    let env_name = "PATH";
    let env_value_old = env!("PATH");
    let env_value_new = format!("{env_value_old}:/some/dir");
    let mut cmd = get_env_cmd(env_name);
    cmd.options_mut().add_env(env_name, &env_value_new);
    let actual_env_value = spawn_and_read_message(cmd).await;

    assert_eq!(env_value_new, actual_env_value.unwrap());
}

fn get_env_cmd(env: &str) -> Cmd {
    let options = CmdOptions::with_logging(LoggingType::StdoutOnly);
    Cmd::with_args_and_options("printenv", [env], options)
}

async fn spawn_and_read_message(runnable: impl Runnable) -> Option<String> {
    let (_dir, handle) = create_process_manager();
    let handle = handle.spawn_with_handle(runnable).await.unwrap();
    let _ = handle.wait(Duration::from_millis(20)).await;
    let logs = handle
        .get_logs_stdout(LogsQuery::fetch_all())
        .await
        .unwrap();
    logs.first().map(|s| s.to_owned())
}
