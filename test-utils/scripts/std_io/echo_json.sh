#/bin/sh
read msg < /dev/stdin
echo "$msg" | jq -r "$1"