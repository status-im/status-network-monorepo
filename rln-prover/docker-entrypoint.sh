#!/bin/bash

args_array=("$@")
#for i in "${args_array[@]}"
#do
#  :
#  echo "### Got variable $i ###"
#done
#echo "args_count = $#"

# export RUST_LOG=debug
exec ./prover_cli "${@}"