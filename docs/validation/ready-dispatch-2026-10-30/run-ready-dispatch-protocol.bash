#!/usr/bin/env bash
set -eo pipefail
source /opt/ros/jazzy/setup.bash
set -u
unset CYCLONEDDS_URI FASTRTPS_DEFAULT_PROFILES_FILE FASTDDS_DEFAULT_PROFILES_FILE
for r2r_rmw in rmw_cyclonedds_cpp rmw_fastrtps_cpp; do
    python3 tools/bench_ros/pubsub_pipeline.py \
        --rust target/ready-dispatch-comparison/r2r-candidate \
        --baseline-rust target/ready-dispatch-comparison/r2r-baseline \
        --cpp target/ready-dispatch-comparison/rclcpp-comparison \
        --cpu-ping 24 --cpu-echo 25 --domain-id 172 --rmw "$r2r_rmw" \
        --cases 0:1:1,4096:16:1,4096:8:4 --iterations 100 --warmup 10 --repetitions 1 \
        --matrix-scope full --rust-receive-path native_callback \
        --baseline-receive-path native_callback \
        --build-manifest target/ready-dispatch-comparison/build-manifest.json \
        --output "target/ready-dispatch-comparison/$r2r_rmw-protocol.json"
    python3 tools/bench_ros/verify_pubsub_pipeline.py --require-current-sources \
        "target/ready-dispatch-comparison/$r2r_rmw-protocol.json"
done
