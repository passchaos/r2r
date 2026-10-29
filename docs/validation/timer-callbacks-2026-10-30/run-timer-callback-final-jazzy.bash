#!/usr/bin/env bash
set -eo pipefail
source /opt/ros/jazzy/setup.bash
set -u
unset CYCLONEDDS_URI FASTRTPS_DEFAULT_PROFILES_FILE FASTDDS_DEFAULT_PROFILES_FILE
export ROS_DOMAIN_ID=176 ROS_LOCALHOST_ONLY=1 RMW_IMPLEMENTATION=rmw_cyclonedds_cpp
rustc --version
cargo --version
cargo fmt --all -- --check
for r2r_rmw in rmw_cyclonedds_cpp rmw_fastrtps_cpp; do
    export RMW_IMPLEMENTATION="$r2r_rmw"
    printf '%s\n' "ROS_DISTRO=$ROS_DISTRO RMW_IMPLEMENTATION=$RMW_IMPLEMENTATION"
    cargo test --locked -p r2r --lib --release callback_timer_ -- --test-threads=1
    cargo run --locked --release -p r2r --example native_timer_callbacks
done
export RMW_IMPLEMENTATION=rmw_cyclonedds_cpp
cargo test --locked --workspace -- --test-threads=1
for r2r_rmw in rmw_cyclonedds_cpp rmw_fastrtps_cpp; do
    export RMW_IMPLEMENTATION="$r2r_rmw"
    printf '%s\n' "ROS_DISTRO=$ROS_DISTRO RMW_IMPLEMENTATION=$RMW_IMPLEMENTATION"
    cargo test --locked -p r2r --lib --release -- --test-threads=1
    cargo run --locked --release -p r2r --example tokio_executor
done
cargo clippy --locked --workspace --all-targets -- -D warnings
