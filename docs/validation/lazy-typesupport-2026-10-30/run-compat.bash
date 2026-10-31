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
   -e CARGO_HOME=/r2r-cargo -e RUSTUP_HOME=/r2r-rustup -e RUSTUP_TOOLCHAIN=1.85.0 -e CARGO_NET_OFFLINE=true \
   -e CARGO_BUILD_JOBS=1 -e RAYON_NUM_THREADS=1 \
   -e CARGO_TARGET_DIR=/target -e ROS_DOMAIN_ID=181 -e ROS_LOCALHOST_ONLY=1 -e R2R_DISTRO="$r2r_distro" -e R2R_RMWS="$r2r_rmws" \
   "$r2r_image" -c '
    set -e
    source "/opt/ros/$R2R_DISTRO/setup.bash"
    unset CYCLONEDDS_URI FASTRTPS_DEFAULT_PROFILES_FILE FASTDDS_DEFAULT_PROFILES_FILE
    export PATH=/r2r-cargo/bin:$PATH
    cd /r2r
    cargo test --offline --locked -p r2r_msg_gen --lib native_ -- --test-threads=1
    for r2r_rmw in $R2R_RMWS; do
      export RMW_IMPLEMENTATION="$r2r_rmw"
      cargo test --offline --locked -p r2r --lib --release msg_types -- --test-threads=1
      cargo test --offline --locked -p r2r --lib --release untyped -- --test-threads=1
      cargo test --offline --locked -p r2r --lib --release type_description -- --test-threads=1
      cargo test --offline --locked -p r2r --lib --release nodes::callback_executor::tests -- --test-threads=1
      cargo run --offline --locked --release -p r2r --example reentrant_callbacks
    done
    cargo clippy --offline --locked --workspace --all-targets -- -D warnings
   '
}
run_humble() {
 run_native humble r2r_humble_validation target/compat-humble-native-ref rmw_fastrtps_cpp > target/lazy-typesupport-final-humble.log 2>&1
}
run_kilted() {
 run_native kilted r2r_kilted_native_feedback_validation target/compat-kilted-native-ref 'rmw_cyclonedds_cpp rmw_fastrtps_cpp' > target/lazy-typesupport-final-kilted.log 2>&1
}
run_no_ros() {
 taskset -c "$r2r_build_cpu" env CARGO_BUILD_JOBS=1 RAYON_NUM_THREADS=1 \
   bash docs/validation/reentrant-groups-2026-10-30/run-no-ros.bash
}
case "${1:-all}" in
 all)
  run_humble
  run_kilted
  run_no_ros
  ;;
 humble) run_humble ;;
 kilted) run_kilted ;;
 no-ros) run_no_ros ;;
 *)
  echo "usage: $0 [all|humble|kilted|no-ros]" >&2
  exit 2
  ;;
esac
