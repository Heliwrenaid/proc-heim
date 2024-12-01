#/bin/sh

cat $INPUT_PIPE | while read msg; do
    echo "$msg" > $OUTPUT_PIPE
done
