use std::time::Duration;

use futures::future::FutureExt as _;
use proc_heim::{
    CmdOptionsBuilder, CustomScriptRunConfig, LoggingType, LogsQuery, MessagingType,
    ProcessManagerHandle, Script, ScriptBuilder, ScriptLanguage, SCRIPT_FILE_PATH_PLACEHOLDER,
};
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::common::create_process_manager;

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
        build_script(ScriptLanguage::Bash, BASH_ECHO_SCRIPT, &args),
        build_script(ScriptLanguage::Python, PYTHON_ECHO_SCRIPT, &args),
        build_script(ScriptLanguage::Perl, PERL_ECHO_SCRIPT, &args),
        build_script(ScriptLanguage::Php, PHP_ECHO_SCRIPT, &args),
        build_script(ScriptLanguage::Ruby, RUBY_ECHO_SCRIPT, &args),
        build_script(ScriptLanguage::Lua, LUA_ECHO_SCRIPT, &args),
        build_script(ScriptLanguage::JavaScript, JAVASCRIPT_ECHO_SCRIPT, &args),
        build_script(ScriptLanguage::Groovy, GROOVY_ECHO_SCRIPT, &args),
    ];

    for script in scripts {
        test_script(&handle, script, arg, &message).await;
    }
}

fn build_script(lang: ScriptLanguage, script: &str, args: &[String]) -> Script {
    ScriptBuilder::default()
        .lang(lang)
        .content(script)
        .args(args)
        .options(
            CmdOptionsBuilder::default()
                .message_input(MessagingType::NamedPipe)
                .message_output(MessagingType::NamedPipe)
                .logging_type(LoggingType::StdoutAndStderr)
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
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
    assert_eq!(format!("First Argument: {arg}"), *stdout.get(0).unwrap());
    assert_eq!(
        format!("Received: {message_to_sent}"),
        *stdout.get(1).unwrap()
    );

    let errors = handle
        .get_logs_stderr(id, LogsQuery::default())
        .await
        .unwrap();

    assert_eq!(1, errors.len());
    assert_eq!(format!("Error: {message_to_sent}"), *errors.get(0).unwrap());

    tokio::time::sleep(Duration::from_secs(1)).await;

    let process_data = handle.get_process_data(id).await.unwrap();
    assert!(process_data.exit_status().unwrap().success());
}

const BASH_ECHO_SCRIPT: &str = r#"
first_arg="$1"
echo "First Argument: $first_arg"

input_pipe="$INPUT_PIPE"
output_pipe="$OUTPUT_PIPE"

read -r line < "$input_pipe"
echo "Received: $line"
echo "Error: $line" >&2

echo "$line" > "$output_pipe"
"#;

const PHP_ECHO_SCRIPT: &str = r#"<?php
$firstArg = isset($argv[1]) ? $argv[1] : "";
echo "First Argument: $firstArg\n";

$inputPipe = getenv('INPUT_PIPE') ?: 'input_pipe';
$outputPipe = getenv('OUTPUT_PIPE') ?: 'output_pipe';

$inputHandle = fopen($inputPipe, 'r');
if (!$inputHandle) {
    die("Unable to open input pipe: $inputPipe\n");
}

$outputHandle = fopen($outputPipe, 'w');
if (!$outputHandle) {
    die("Unable to open output pipe: $outputPipe\n");
}

while (true) {
    $line = fgets($inputHandle);
    if ($line !== false) {
        $line = rtrim($line);
        echo "Received: $line\n";
        fwrite($outputHandle, $line . PHP_EOL);
        fwrite(STDERR, "Error: $line\n");
        break;
    } else {
        usleep(100000); // 100 ms
    }
}

fclose($inputHandle);
fclose($outputHandle);
"#;

const PYTHON_ECHO_SCRIPT: &str = r#"
import os
import sys
import time

first_arg = sys.argv[1] if len(sys.argv) > 1 else ""
print(f"First Argument: {first_arg}")

input_pipe = os.environ.get("INPUT_PIPE")
output_pipe = os.environ.get("OUTPUT_PIPE")

with open(input_pipe, 'r') as input_file:
    with open(output_pipe, 'w') as output_file:
        while True:
            line = input_file.readline()
            if line:
                line = line.strip()

                print(f"Received: {line}")
                sys.stdout.flush()

                print(f"Error: {line}", file=sys.stderr)
                sys.stderr.flush()

                output_file.write(line + '\n')
                output_file.flush()
                break
               
            else:
                time.sleep(0.1)  # 100 ms
"#;

const GROOVY_ECHO_SCRIPT: &str = r#"
def firstArg = args.length > 0 ? args[0] : ""
println "First Argument: $firstArg"

def inputPipe = System.getenv("INPUT_PIPE") ?: "input_pipe"
def outputPipe = System.getenv("OUTPUT_PIPE") ?: "output_pipe"

def process = new ProcessBuilder("cat", inputPipe).redirectErrorStream(true).start()
def reader = new BufferedReader(new InputStreamReader(process.getInputStream()))
def line = reader.readLine()

println "Received: $line"
System.err.println "Error: $line"
new File(outputPipe).text = line
"#;

const RUBY_ECHO_SCRIPT: &str = r#"
STDOUT.sync = true
STDERR.sync = true

first_arg = ARGV[0] || ""
puts "First Argument: #{first_arg}"

input_pipe = ENV["INPUT_PIPE"] || "input_pipe"
output_pipe = ENV["OUTPUT_PIPE"] || "output_pipe"

File.open(input_pipe, "r") do |input|
  line = input.gets.chomp
  puts "Received: #{line}"
  STDOUT.flush
  warn "Error: #{line}"
  File.open(output_pipe, "w") { |output| output.puts line }
end
"#;

const LUA_ECHO_SCRIPT: &str = r#"
local first_arg = arg[1] or ""
print("First Argument: " .. first_arg)

local input_pipe = os.getenv("INPUT_PIPE") or "input_pipe"
local output_pipe = os.getenv("OUTPUT_PIPE") or "output_pipe"

local input_file = io.open(input_pipe, "r")
local line = input_file:read("*line")
input_file:close()
print("Received: " .. line)
io.stderr:write("Error: " .. line .. "\n")

local output_file = io.open(output_pipe, "w")
output_file:write(line)
output_file:close()
"#;

const PERL_ECHO_SCRIPT: &str = r#"
use strict;
use warnings;

$| = 1; # Turn off buffering for STDOUT

my $first_arg = $ARGV[0] // '';
print "First Argument: $first_arg\n";

my $input_pipe = $ENV{'INPUT_PIPE'};
my $output_pipe = $ENV{'OUTPUT_PIPE'};

open my $input_fh, '<', $input_pipe or die "Cannot open input pipe: $!";
my $line = <$input_fh>;
chomp $line;
print STDOUT "Received: $line\n";
print STDERR "Error: $line\n";

open my $output_fh, '>', $output_pipe or die "Cannot open output pipe: $!";
print $output_fh $line;
close $output_fh;
"#;

const JAVASCRIPT_ECHO_SCRIPT: &str = r#"
const fs = require('fs');

const firstArg = process.argv[2] || "";
console.log(`First Argument: ${firstArg}`);

const inputPipe = process.env.INPUT_PIPE;
const outputPipe = process.env.OUTPUT_PIPE;

fs.open(inputPipe, 'r', (err, fd) => {
    if (err) {
        console.error(`Error opening pipe: ${err.message}`);
        return;
    }

    const buffer = Buffer.alloc(1024);
    fs.read(fd, buffer, 0, buffer.length, null, (err, bytesRead) => {
        if (err) {
            console.error(`Error reading pipe: ${err.message}`);
            return;
        }

        const line = buffer.toString('utf8', 0, bytesRead).trim();
        console.log(`Received: ${line}`);
        console.error(`Error: ${line}`);
        fs.writeFileSync(outputPipe, line, 'utf-8');
    });
});
"#;
