# Proc-heim

`Proc-heim` is a library for running and managing short-lived and long-lived processes using asynchronous API. A new process can be created by running either command or script. `Proc-heim` is hosted on
[crates.io](https://crates.io/crates/proc-heim), with API
Documentation on [docs.rs](https://docs.rs/proc-heim/).

## Features
`Proc-heim` internally uses [tokio::process](https://docs.rs/tokio/latest/tokio/process/index.html) for executing processes and provides all its functionality plus additional features:
 * spawning new processes via scripts (in different scripting languages) and any `Rust` types, which implements `Runnable` trait,
 * flexible managing of all spawned processes using single facade, which can be easily shared by multiple threads/tasks,
 * bi-directional, message-based communication between child and parent processes via standard IO streams or named pipes,
 * collecting and querying logs produced by child processes (both running and completed).

For more detailed list of features see [ProcessManagerHandle](https://docs.rs/proc-heim/latest/proc_heim/manager/struct.ProcessManagerHandle.html) documentation.

## API overview
`Proc-heim` library is divided into two modules: `model` and `manager`. 

The first one defines types and traits used to describe commands, scripts and their settings, such as  messaging and logging type, environment variables and working directory. The module contains a `Runnable` trait, which defines how to run a user-defined process. The library provides two implementation of this trait: `Cmd` and `Script`.

The `manager` module provides an API for spawning and managing child processes. The whole implementation relies on `Actor model` architecture. To start using the library, a client code needs to spawn a `ProcessManager` task, responsible for:
* creating new actors implementing some functionality (eg. reading messages from child process),
* forwarding messages sent between client code and other actors.

After spawning the `ProcessManager` task, a `ProcessManagerHandle` is being returned, which exposes an API for spawning and managing user-defined processes.

## Examples

### Spawning a new `ProcessManager` task
```rust
use proc_heim::manager::ProcessManager;
use std::path::PathBuf;

let working_directory = PathBuf::from("/some/temp/path");
let handle = ProcessManager::spawn(working_directory)?;
// now use the handle to spawn new processes and interact with them
```

### Spawning a new process from command
```rust
use proc_heim::manager::ProcessManager;
use std::path::PathBuf;

let working_directory = PathBuf::from("/tmp/proc_heim");
let handle = ProcessManager::spawn(working_directory)?;
let cmd = Cmd::with_args("ls", ["-l", "/some/dir"]);
let process_id = handle.spawn(cmd).await?;
// now use the process_id to interact with a process ...
```

### Reading logs from a process
```rust
use proc_heim::{
    manager::{LogsQuery, ProcessManager},
    model::{
        command::{CmdOptions, LoggingType},
        script::{Script, ScriptingLanguage}
    },
};
use std::{path::PathBuf, time::Duration};

let working_directory = PathBuf::from("/tmp/proc_heim");
    let handle = ProcessManager::spawn(working_directory)?;
    let script = Script::with_args_and_options(
        ScriptingLanguage::Bash,
        r#"
        echo 'Simple log example'
        echo "Hello $1"
        echo 'Error log' >&2
        "#,
        ["World"],
        CmdOptions::with_logging(LoggingType::StdoutAndStderr),
    );

    let process_id = handle.spawn(script).await?;
    // We are waiting for the process to exit in order to get all logs
    handle.wait(process_id, Duration::from_micros(10)).await??;

    let logs = handle
        .get_logs_stdout(process_id, LogsQuery::with_offset(1))
        .await?;
    assert_eq!(1, logs.len());
    assert_eq!("Hello World", logs[0]);

    let error_logs = handle
        .get_logs_stderr(process_id, LogsQuery::fetch_all())
        .await?;
    assert_eq!(1, error_logs.len());
    assert_eq!("Error log", error_logs[0]);
```

### Messaging with a process via named pipes

```rust
use futures::TryStreamExt;
use proc_heim::{
    manager::ProcessManager,
    model::{
        command::CmdOptions,
        script::{Script, ScriptingLanguage},
    },
};
use std::path::PathBuf;

let working_directory = PathBuf::from("/tmp/proc_heim");
let handle = ProcessManager::spawn(working_directory)?;
let script = Script::with_options(
    ScriptingLanguage::Bash,
    r#"
    counter=0
    while read msg; do
        echo "$counter: $msg" > $OUTPUT_PIPE
        counter=$((counter + 1))
    done < $INPUT_PIPE
    "#,
    CmdOptions::with_named_pipe_messaging(), // we want to send messages bidirectionally
);

// We can use "spawn_with_handle" instead of "spawn" to get "ProcessHandle",
// which mimics the "ProcessManagerHandle" API, 
// but without having to pass the process ID to each method call.
let process_handle = handle.spawn_with_handle(script).await?;

process_handle.send_message("First message").await?;
// We can send a next message without causing a deadlock here.
// This is possible because the response to the first message
// will be read by a dedicated Tokio task, 
// spawned automatically by the Process Manager.
process_handle.send_message("Second message").await?;

let mut stream = process_handle.subscribe_message_string_stream().await?;

let msg = stream.try_next().await?.unwrap();
assert_eq!("0: First message", msg);

let msg = stream.try_next().await?.unwrap();
assert_eq!("1: Second message", msg);

let result = process_handle.kill().await;
assert!(result.is_ok());
```

For more examples, see [integration tests](https://github.com/Heliwrenaid/proc-heim/tree/main/tests).

## License
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in <b>proc-heim</b> by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
