#/bin/sh
read msg < $INPUT_PIPE
echo "$msg" | jq -r "$1" > $OUTPUT_PIPE