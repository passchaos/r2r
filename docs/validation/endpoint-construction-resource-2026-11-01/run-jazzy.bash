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
export ROS_DOMAIN_ID=227 ROS_LOCALHOST_ONLY=1

cargo fmt --all -- --check
r2r_cpu="$(python3 -c 'import os; print(min(os.sched_getaffinity(0)))')"
for r2r_rmw in rmw_cyclonedds_cpp rmw_fastrtps_cpp; do
  export RMW_IMPLEMENTATION="$r2r_rmw"
  python3 tools/bench_ros/endpoint_construction_resource_stage.py run \
    --probe target/native-action-feedback-probe/normal/libr2r_bench_probe.so \
    --cpu "$r2r_cpu" --repetitions 5 \
    --output "docs/validation/endpoint-construction-resource-2026-11-01/allocation-stage-${r2r_rmw#rmw_}.json"
done
cargo clippy --offline --locked -p r2r --lib --tests -- -D warnings
