#/bin/sh

cat $INPUT_PIPE | while read msg; do
    echo "Echo: $msg" > $OUTPUT_PIPE
done
