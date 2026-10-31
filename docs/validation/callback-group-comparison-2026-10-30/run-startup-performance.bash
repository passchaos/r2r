#!/usr/bin/env bash
set -eo pipefail
ulimit -v 10485760
source /opt/ros/jazzy/setup.bash
set -u
unset CYCLONEDDS_URI FASTRTPS_DEFAULT_PROFILES_FILE FASTDDS_DEFAULT_PROFILES_FILE LD_PRELOAD
for r2r_rmw in rmw_cyclonedds_cpp rmw_fastrtps_cpp; do
    python3 tools/bench_ros/callback_group_startup.py \
        --rust target/callback-group-comparison/r2r-candidate \
        --cpp target/callback-group-comparison/rclcpp-comparison \
        --build-manifest target/callback-group-comparison/build-manifest.json \
        --cpus 24,25,26,27 --case exclusive:1:1:0 --repetitions 30 \
        --rmw "$r2r_rmw" --domain-id 185 \
        --isolation-samples 3 --isolation-interval 10 --isolation-timeout 50 --isolation-max-load1 2 \
        --max-rclcpp-ratio 1.0 --require-performance-pass \
        --output "target/callback-group-startup-performance-$r2r_rmw.json"
done
