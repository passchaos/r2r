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
export ROS_DOMAIN_ID=188 ROS_LOCALHOST_ONLY=1 RMW_IMPLEMENTATION=rmw_cyclonedds_cpp
cargo fmt --all -- --check
cargo build --offline --locked -p r2r --example graceful_shutdown
cargo test --offline --locked -p r2r --test context_shutdown -- --test-threads=1
cargo clippy --offline --locked -p r2r --example graceful_shutdown -- -D warnings

r2r_output="$(mktemp)"
r2r_pid=""
cleanup() {
  if [[ -n "$r2r_pid" ]] && kill -0 "$r2r_pid" 2>/dev/null; then
    kill -TERM "$r2r_pid" 2>/dev/null || true
    wait "$r2r_pid" 2>/dev/null || true
  fi
  rm -f "$r2r_output"
}
trap cleanup EXIT
target/debug/examples/graceful_shutdown >"$r2r_output" 2>&1 &
r2r_pid=$!
for _ in $(seq 1 100); do
  if grep -q '^ready; send SIGINT or SIGTERM$' "$r2r_output"; then
    break
  fi
  if ! kill -0 "$r2r_pid" 2>/dev/null; then
    cat "$r2r_output"
    exit 1
  fi
  sleep 0.05
done
grep -q '^ready; send SIGINT or SIGTERM$' "$r2r_output"
kill -INT "$r2r_pid"
wait "$r2r_pid"
r2r_pid=""
grep -q '^shutdown complete; recovered 1 node(s)$' "$r2r_output"
cat "$r2r_output"
