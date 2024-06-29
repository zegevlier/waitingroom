#/bin/bash

echo -e "target\ttotal\tkilled\tkendall_tau\ttime_taken"
for file in ./results/*.jsonl; do
    TEST_NAME=$(echo -n $file | sed 's/.*\///' | sed 's/\.jsonl//')
    echo -n $TEST_NAME | sed -E 's/target_([0-9]*)_total_([0-9]*)_killed_([0-9]*).*/\1\t\2\t\3\t/'
    jq -js 'map(.kendall_tau) | add / length' $file
    echo -en "\t"
    jq -s 'map(.time_taken) | add / length' $file
done