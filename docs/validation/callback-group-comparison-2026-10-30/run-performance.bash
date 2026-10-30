#!/usr/bin/env bash
set -eo pipefail
source /opt/ros/jazzy/setup.bash
set -u
unset CYCLONEDDS_URI FASTRTPS_DEFAULT_PROFILES_FILE FASTDDS_DEFAULT_PROFILES_FILE
for r2r_rmw in rmw_cyclonedds_cpp rmw_fastrtps_cpp; do
    python3 tools/bench_ros/callback_groups.py \
        --rust target/callback-group-comparison/r2r-candidate --cpp target/callback-group-comparison/rclcpp-comparison \
        --probe target/callback-group-comparison/libr2r_bench_probe.so --build-manifest target/callback-group-comparison/build-manifest.json \
        --cpus 24,25,26,27 --iterations 1000 --warmup 100 --repetitions 5 --rmw "$r2r_rmw" \
        --isolation-samples 3 --isolation-interval 10 --isolation-timeout 50 --isolation-max-load1 2 \
        --max-rclcpp-ratio 1.0 --require-performance-pass --output "target/callback-group-performance-$r2r_rmw.json"
done
