#/bin/bash

for file in ./results/*.jsonl; do
    echo -n $file " " | sed 's/.*\///' | sed 's/\.jsonl//' 
    jq -s 'map(.kendall_tau) | add / length' $file
done