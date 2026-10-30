#!/usr/bin/env bash
set -eo pipefail
source /opt/ros/jazzy/setup.bash
set -u
unset CYCLONEDDS_URI FASTRTPS_DEFAULT_PROFILES_FILE FASTDDS_DEFAULT_PROFILES_FILE
export ROS_DOMAIN_ID=182 ROS_LOCALHOST_ONLY=1 RMW_IMPLEMENTATION=rmw_cyclonedds_cpp
rustc --version
cargo --version
cargo fmt --all -- --check
for r2r_rmw in rmw_cyclonedds_cpp rmw_fastrtps_cpp; do
    export RMW_IMPLEMENTATION="$r2r_rmw"
    printf '%s\n' "ROS_DISTRO=$ROS_DISTRO RMW_IMPLEMENTATION=$RMW_IMPLEMENTATION"
    cargo test --locked --offline -p r2r --lib callback_group_ -- --test-threads=1
    cargo test --locked --offline -p r2r --lib --release callback_group_ -- --test-threads=1
    cargo run --locked --offline --release -p r2r --example callback_groups
done
export RMW_IMPLEMENTATION=rmw_cyclonedds_cpp
cargo test --locked --offline -p r2r --doc CallbackExecutor
cargo test --locked --offline -p r2r --doc subscribe_native_callback_in_group
cargo test --locked --offline --workspace -- --test-threads=1
for r2r_rmw in rmw_cyclonedds_cpp rmw_fastrtps_cpp; do
    export RMW_IMPLEMENTATION="$r2r_rmw"
    printf '%s\n' "ROS_DISTRO=$ROS_DISTRO RMW_IMPLEMENTATION=$RMW_IMPLEMENTATION"
    cargo test --locked --offline -p r2r --lib --release -- --test-threads=1
done
cargo clippy --locked --offline --workspace --all-targets -- -D warnings
