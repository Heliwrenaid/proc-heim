#/bin/sh

while read msg; do
    echo "Echo: $msg"
done < /dev/stdin
