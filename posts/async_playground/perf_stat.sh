#! /usr/bin/env bash

cargo build --release

perf stat -e task-clock ./target/release/sleep_busy

# Performance counter stats for './target/release/sleep_busy':
#
#          1,001.77 msec task-clock                       #    1.000 CPUs utilized
#
#       1.001748642 seconds time elapsed
#
#       1.002047000 seconds user
#       0.000000000 seconds sys

perf stat -e task-clock ./target/release/foo

# Performance counter stats for './target/release/foo':
#
#              2.08 msec task-clock                       #    0.002 CPUs utilized
#
#       1.002680849 seconds time elapsed
#
#       0.000000000 seconds user
#       0.002491000 seconds sys
