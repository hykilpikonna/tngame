#!/usr/bin/env bash
sigint_handler()
{
  kill $PID
  exit
}

trap sigint_handler SIGINT

while true; do
  cargo run &
  PID=$!
  inotifywait -e modify -e move -e create -e delete -e attrib -r `pwd` --include '.*\.rs'
  kill $PID
done

