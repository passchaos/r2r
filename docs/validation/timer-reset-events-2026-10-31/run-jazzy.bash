#!/usr/bin/env bash
set -eo pipefail
source /opt/ros/jazzy/setup.bash
set -u
ulimit -v 10485760
export CARGO_BUILD_JOBS=1 RAYON_NUM_THREADS=1 ROS_DOMAIN_ID=220 ROS_LOCALHOST_ONLY=1
unset CYCLONEDDS_URI FASTRTPS_DEFAULT_PROFILES_FILE FASTDDS_DEFAULT_PROFILES_FILE

cargo fmt --all -- --check
for r2r_rmw in rmw_cyclonedds_cpp rmw_fastrtps_cpp; do
    export RMW_IMPLEMENTATION="$r2r_rmw"
    printf '%s\n' "ROS_DISTRO=$ROS_DISTRO RMW_IMPLEMENTATION=$RMW_IMPLEMENTATION"
    taskset -c 0 cargo test --locked -p r2r --lib reset_events -- --test-threads=1
    taskset -c 0 cargo test --locked -p r2r --lib callback_and_grouped_timers_share_reset_event_contract -- --test-threads=1
    taskset -c 0 cargo test --locked -p r2r --lib --release reset_events -- --test-threads=1
    taskset -c 0 cargo test --locked -p r2r --lib --release callback_and_grouped_timers_share_reset_event_contract -- --test-threads=1
    taskset -c 0 cargo run --locked --release -p r2r --example timer_reset_events
done
taskset -c 0 cargo clippy --locked --workspace --all-targets -- -D warnings
