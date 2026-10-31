#!/usr/bin/env bash
set -eo pipefail
if [[ -z "${R2R_SINGLE_CORE_ACTIVE:-}" ]]; then
  export R2R_SINGLE_CORE_ACTIVE=1
  r2r_build_cpu="${R2R_BUILD_CPU:-$(python3 -c 'import os; print(min(os.sched_getaffinity(0)))')}"
  exec taskset -c "$r2r_build_cpu" bash "$0" "$@"
fi
ulimit -v 10485760
source /opt/ros/jazzy/setup.bash
set -u
unset CYCLONEDDS_URI FASTRTPS_DEFAULT_PROFILES_FILE FASTDDS_DEFAULT_PROFILES_FILE
export CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true RAYON_NUM_THREADS=1 RUST_TEST_THREADS=1
export ROS_DOMAIN_ID=186 ROS_LOCALHOST_ONLY=1 RMW_IMPLEMENTATION=rmw_cyclonedds_cpp
cargo fmt --all -- --check
cargo check --offline --locked -p r2r --example parameter_callbacks
cargo test --offline --locked -p r2r --lib parameter_ -- --test-threads=1
cargo clippy --offline --locked -p r2r --example parameter_callbacks -- -D warnings
