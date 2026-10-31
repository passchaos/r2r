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
export ROS_DOMAIN_ID=190 ROS_LOCALHOST_ONLY=1

cargo fmt --all -- --check
for r2r_rmw in rmw_cyclonedds_cpp rmw_fastrtps_cpp; do
  export RMW_IMPLEMENTATION="$r2r_rmw"
  cargo test --offline --locked -p r2r --lib action_clients:: -- --test-threads=1
  cargo test --offline --locked -p r2r --lib \
    typed_native_feedback_stream_delivers_the_taken_message_and_closes_at_result \
    -- --test-threads=1
  cargo run --offline --locked -p r2r --example action_native_feedback
done
cargo clippy --offline --locked -p r2r --lib --example action_native_feedback -- -D warnings
