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
export ROS_DOMAIN_ID=212 ROS_LOCALHOST_ONLY=1

cargo fmt --all -- --check
for r2r_rmw in rmw_cyclonedds_cpp rmw_fastrtps_cpp; do
  export RMW_IMPLEMENTATION="$r2r_rmw"
  cargo test --offline --locked -p r2r --lib generated_native_update -- --test-threads=1
  cargo test --offline --locked -p r2r --lib reusable_typed_stream -- --test-threads=1
  cargo test --offline --locked -p r2r --lib \
    default_typed_stream_reuses_native_decode_storage_behind_owned_samples -- --test-threads=1
  cargo test --offline --locked --release -p r2r --lib generated_native_update -- --test-threads=1
  cargo test --offline --locked --release -p r2r --lib reusable_typed_stream -- --test-threads=1
  cargo test --offline --locked --release -p r2r --lib \
    default_typed_stream_reuses_native_decode_storage_behind_owned_samples -- --test-threads=1
  cargo run --offline --locked -p r2r --example reusable_typed_subscription
done
cargo clippy --offline --locked -p r2r_msg_gen --lib -- -D warnings
cargo clippy --offline --locked -p r2r --lib --example reusable_typed_subscription -- -D warnings
