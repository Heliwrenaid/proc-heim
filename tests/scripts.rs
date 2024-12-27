use std::time::Duration;

use futures::future::FutureExt as _;
use proc_heim::{
    CmdOptionsBuilder, CustomScriptRunConfig, LogsQuery, MessagingType, ProcessManagerHandle,
    Script, ScriptBuilder, ScriptLanguage, SCRIPT_FILE_PATH_PLACEHOLDER,
};
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::common::create_process_manager;
use test_utils::scripts_collection::*;

mod common;

#[tokio::test]
async fn should_run_custom_script() {
    let (dir, handle) = create_process_manager();

    let run_config = CustomScriptRunConfig::new(
        "bash",
        vec!["-C".into(), SCRIPT_FILE_PATH_PLACEHOLDER.into()],
        "sh",
    );
    let arg = Uuid::new_v4().to_string();
    let script = ScriptBuilder::default()
        .lang(ScriptLanguage::Other(run_config))
        .content(
            r#"
                dir="$(pwd)/$1"
                mkdir $dir
                echo $dir
            "#,
        )
        .args(vec![arg.to_owned()])
        .options(
            CmdOptionsBuilder::default()
                .message_output(MessagingType::StandardIo)
                .current_dir(dir.path())
                .build()
                .unwrap(),
        )
        .build()
        .unwrap();

    let id = handle.spawn(script).await.unwrap();
    let mut stream = handle.subscribe_message_bytes_stream(id).await.unwrap();
    let message = stream.try_next().await.unwrap().unwrap();
    assert_eq!(
        dir.path()
            .join(arg)
            .to_path_buf()
            .to_str()
            .unwrap()
            .as_bytes(),
        message
    );
}

#[tokio::test]
async fn test_scripts_in_different_languages() {
    let (_dir, handle) = create_process_manager();

    let arg: &str = "example argument with spaces";
    let args = vec![arg.to_owned()];
    let message = "Test message";

    let scripts = [
        build_echo_script(ScriptLanguage::Bash, BASH_ECHO_SCRIPT, &args),
        build_echo_script(ScriptLanguage::Python, PYTHON_ECHO_SCRIPT, &args),
        build_echo_script(ScriptLanguage::Perl, PERL_ECHO_SCRIPT, &args),
        build_echo_script(ScriptLanguage::Php, PHP_ECHO_SCRIPT, &args),
        build_echo_script(ScriptLanguage::Ruby, RUBY_ECHO_SCRIPT, &args),
        build_echo_script(ScriptLanguage::Lua, LUA_ECHO_SCRIPT, &args),
        build_echo_script(ScriptLanguage::JavaScript, JAVASCRIPT_ECHO_SCRIPT, &args),
        build_echo_script(ScriptLanguage::Groovy, GROOVY_ECHO_SCRIPT, &args),
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
    tokio::time::sleep(Duration::from_secs(1)).await;

    handle.write_message(id, message_to_sent).await.unwrap();

    let mut stream = handle.subscribe_message_bytes_stream(id).await.unwrap();
    let message = stream.try_next().await.unwrap().unwrap();
    assert_eq!(message_to_sent.as_bytes(), message);
    assert!(stream.next().now_or_never().is_none());

    let stdout = handle
        .get_logs_stdout(id, LogsQuery::default())
        .await
        .unwrap();

    assert_eq!(2, stdout.len());
    assert_eq!(format!("First Argument: {arg}"), *stdout.first().unwrap());
    assert_eq!(
        format!("Received: {message_to_sent}"),
        *stdout.get(1).unwrap()
    );

    let errors = handle
        .get_logs_stderr(id, LogsQuery::default())
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
        .await
        .unwrap()
        .unwrap();
    assert!(process_data.exit_status().unwrap().success());
}
