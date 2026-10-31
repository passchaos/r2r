#!/usr/bin/env bash
set -euo pipefail
ulimit -v 10485760
r2r_build_cpu="${R2R_BUILD_CPU:-$(python3 -c 'import os; print(min(os.sched_getaffinity(0)))')}"

run_native() {
  local r2r_distro="$1" r2r_image="$2" r2r_target="$3" r2r_rmws="$4"
  mkdir -p "$r2r_target"
  docker run --rm --entrypoint bash \
    --cpuset-cpus "$r2r_build_cpu" --memory 10g --memory-swap 10g \
    -v "$PWD:/r2r:ro" -v "$PWD/$r2r_target:/target" \
    -v /home/passchaos/.cargo:/r2r-cargo:ro -v /home/passchaos/.rustup:/r2r-rustup:ro \
    -e CARGO_HOME=/r2r-cargo -e RUSTUP_HOME=/r2r-rustup -e RUSTUP_TOOLCHAIN=1.85.0 \
    -e CARGO_BUILD_JOBS=1 -e CARGO_NET_OFFLINE=true -e RAYON_NUM_THREADS=1 \
    -e RUST_TEST_THREADS=1 -e CARGO_TARGET_DIR=/target -e ROS_DOMAIN_ID=215 \
    -e ROS_LOCALHOST_ONLY=1 -e R2R_DISTRO="$r2r_distro" -e R2R_RMWS="$r2r_rmws" \
    "$r2r_image" -c '
      set -e
      source "/opt/ros/$R2R_DISTRO/setup.bash"
      unset CYCLONEDDS_URI FASTRTPS_DEFAULT_PROFILES_FILE FASTDDS_DEFAULT_PROFILES_FILE
      export PATH=/r2r-cargo/bin:$PATH
      cd /r2r
      for r2r_rmw in $R2R_RMWS; do
        export RMW_IMPLEMENTATION="$r2r_rmw"
        cargo test --offline --locked -p r2r --lib \
          default_typed_client_reuses_request_storage_and_retires_failed_copies \
          -- --test-threads=1
      done
      cargo clippy --offline --locked -p r2r --lib -- -D warnings
    '
}

run_native humble r2r_humble_validation target/typed-client-request-native-reuse-humble \
  rmw_fastrtps_cpp > target/typed-client-request-native-reuse-final-humble.log 2>&1
run_native kilted r2r_kilted_native_feedback_validation target/typed-client-request-native-reuse-kilted \
  'rmw_cyclonedds_cpp rmw_fastrtps_cpp' > target/typed-client-request-native-reuse-final-kilted.log 2>&1
