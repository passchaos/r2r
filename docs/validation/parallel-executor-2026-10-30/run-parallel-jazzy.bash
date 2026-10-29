#!/usr/bin/env bash
set -eo pipefail
source /opt/ros/jazzy/setup.bash
set -u
unset CYCLONEDDS_URI FASTRTPS_DEFAULT_PROFILES_FILE FASTDDS_DEFAULT_PROFILES_FILE
export ROS_DOMAIN_ID=180 ROS_LOCALHOST_ONLY=1 RMW_IMPLEMENTATION=rmw_cyclonedds_cpp
rustc --version
cargo --version
cargo fmt --all -- --check
for r2r_rmw in rmw_cyclonedds_cpp rmw_fastrtps_cpp; do
    export RMW_IMPLEMENTATION="$r2r_rmw"
    printf '%s\n' "ROS_DISTRO=$ROS_DISTRO RMW_IMPLEMENTATION=$RMW_IMPLEMENTATION"
    cargo test --locked -p r2r --lib parallel_node_executor_ -- --test-threads=1
    cargo run --locked -p r2r --example parallel_executor
done
cargo clippy --locked -p r2r --lib --tests --example parallel_executor -- -D warnings
cargo test --locked -p r2r --doc ParallelExecutor
for r2r_rmw in rmw_cyclonedds_cpp rmw_fastrtps_cpp; do
    export RMW_IMPLEMENTATION="$r2r_rmw"
    printf '%s\n' "ROS_DISTRO=$ROS_DISTRO RMW_IMPLEMENTATION=$RMW_IMPLEMENTATION"
    cargo test --locked -p r2r --lib --release parallel_node_executor_ -- --test-threads=1
    cargo run --locked --release -p r2r --example parallel_executor
done
export RMW_IMPLEMENTATION=rmw_cyclonedds_cpp
cargo test --locked --workspace -- --test-threads=1
for r2r_rmw in rmw_cyclonedds_cpp rmw_fastrtps_cpp; do
    export RMW_IMPLEMENTATION="$r2r_rmw"
    printf '%s\n' "ROS_DISTRO=$ROS_DISTRO RMW_IMPLEMENTATION=$RMW_IMPLEMENTATION"
    cargo test --locked -p r2r --lib --release -- --test-threads=1
done
cargo clippy --locked --workspace --all-targets -- -D warnings
