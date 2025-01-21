use std::time::Duration;

use futures::future::FutureExt as _;
use proc_heim::{
    manager::{LogsQuery, ProcessManagerHandle},
    model::{
        command::{CmdOptions, MessagingType},
        script::{Script, ScriptRunConfig, ScriptingLanguage, SCRIPT_FILE_PATH_PLACEHOLDER},
    },
};
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::common::create_process_manager;
use test_utils::scripts_collection::*;

mod common;

#[tokio::test]
async fn should_run_custom_script() {
    let (dir, handle) = create_process_manager();

    let run_config = ScriptRunConfig::new("bash", vec!["-C", SCRIPT_FILE_PATH_PLACEHOLDER], "sh");
    let arg = Uuid::new_v4().to_string();

    let mut options = CmdOptions::with_message_output(MessagingType::StandardIo);
    options.set_current_dir(dir.path().into());

    let script = Script::with_args_and_options(
        ScriptingLanguage::Other(run_config),
        r#"
        dir="$(pwd)/$1"
        mkdir $dir
        echo $dir
        "#,
        [&arg],
        options,
    );

    let id = handle.spawn(script).await.unwrap();
    let mut stream = handle.subscribe_message_stream(id).await.unwrap();
    let message = stream.try_next().await.unwrap().unwrap();
    assert_eq!(
        dir.path().join(arg).to_path_buf().to_str().unwrap(),
        message.try_into_string().unwrap()
    );
}

// TODO: add test also for messaging via StandardIo
#[tokio::test]
async fn test_scripts_in_different_languages() {
    let (_dir, handle) = create_process_manager();

    let arg: &str = "example argument with spaces";
    let args = vec![arg.to_owned()];
    let message = "Test message";

    let scripts = [
        build_echo_script(ScriptingLanguage::Bash, BASH_ECHO_SCRIPT, &args),
        build_echo_script(ScriptingLanguage::Python, PYTHON_ECHO_SCRIPT, &args),
        build_echo_script(ScriptingLanguage::Perl, PERL_ECHO_SCRIPT, &args),
        build_echo_script(ScriptingLanguage::Php, PHP_ECHO_SCRIPT, &args),
        build_echo_script(ScriptingLanguage::Ruby, RUBY_ECHO_SCRIPT, &args),
        build_echo_script(ScriptingLanguage::Lua, LUA_ECHO_SCRIPT, &args),
        build_echo_script(ScriptingLanguage::JavaScript, JAVASCRIPT_ECHO_SCRIPT, &args),
        build_echo_script(ScriptingLanguage::Groovy, GROOVY_ECHO_SCRIPT, &args),
    ];

    for script in scripts {
        test_script(&handle, script, arg, message).await;
    }
}

async fn test_script(
    handle: &ProcessManagerHandle,
    script: Script,
    arg: &str,
    message_to_sent: &str,
) {
    let id = handle.spawn(script.clone()).await.unwrap();

    handle.send_message(id, message_to_sent).await.unwrap();

    let mut stream = handle.subscribe_message_stream(id).await.unwrap();
    let message = stream.try_next().await.unwrap().unwrap();
    assert_eq!(message_to_sent.as_bytes(), &message.into_bytes());
    assert!(stream.next().now_or_never().is_none());

    let stdout = handle
        .get_logs_stdout(id, LogsQuery::fetch_all())
        .await
        .unwrap();

    assert_eq!(2, stdout.len());
    assert_eq!(format!("First Argument: {arg}"), *stdout.first().unwrap());
    assert_eq!(
        format!("Received: {message_to_sent}"),
        *stdout.get(1).unwrap()
    );

    let errors = handle
        .get_logs_stderr(id, LogsQuery::fetch_all())
        .await
        .unwrap();

    assert_eq!(1, errors.len());
    assert_eq!(
        format!("Error: {message_to_sent}"),
        *errors.first().unwrap()
    );

    let process_data = handle
        .wait(id, Duration::from_millis(500))
        .await
        .unwrap()
        .unwrap();
    assert!(process_data.exit_status().unwrap().success());
}
