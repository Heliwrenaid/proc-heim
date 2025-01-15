use proc_heim::model::{
    command::{CmdOptions, LoggingType},
    script::{Script, ScriptingLanguage},
};

pub fn build_echo_script(lang: ScriptingLanguage, script: &str, args: &[String]) -> Script {
    let mut options = CmdOptions::with_named_pipe_messaging();
    options
        .set_logging_type(LoggingType::StdoutAndStderr)
        .unwrap();
    Script::with_args_and_options(lang, script, args, options)
}

pub const BASH_ECHO_SCRIPT: &str = r#"
first_arg="$1"
echo "First Argument: $first_arg"

input_pipe="$INPUT_PIPE"
output_pipe="$OUTPUT_PIPE"

read -r line < "$input_pipe"
echo "Received: $line"
echo "Error: $line" >&2

echo "$line" > "$output_pipe"
"#;

pub const PHP_ECHO_SCRIPT: &str = r#"<?php
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

pub const PYTHON_ECHO_SCRIPT: &str = r#"
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

pub const GROOVY_ECHO_SCRIPT: &str = r#"
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

pub const RUBY_ECHO_SCRIPT: &str = r#"
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

pub const LUA_ECHO_SCRIPT: &str = r#"
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

pub const PERL_ECHO_SCRIPT: &str = r#"
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

pub const JAVASCRIPT_ECHO_SCRIPT: &str = r#"
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
