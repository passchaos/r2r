#!/usr/bin/env bash
set -euo pipefail
ulimit -v 10485760

run_ros_container() {
    local r2r_distro="$1" r2r_image="$2" r2r_target="$3" r2r_rmws="$4" r2r_domain="$5"
    mkdir -p "$r2r_target"
    docker run --rm --cpuset-cpus 0 --memory 10g --memory-swap 10g --entrypoint bash \
        -v "$PWD:/r2r:ro" -v "$PWD/$r2r_target:/target" \
        -v /home/passchaos/.cargo:/r2r-cargo:ro -v /home/passchaos/.rustup:/r2r-rustup:ro \
        -e CARGO_HOME=/r2r-cargo -e RUSTUP_HOME=/r2r-rustup -e RUSTUP_TOOLCHAIN=1.85.0 \
        -e CARGO_TARGET_DIR=/target -e CARGO_BUILD_JOBS=1 -e RAYON_NUM_THREADS=1 \
        -e ROS_LOG_DIR=/tmp/r2r-timer-reset-logs -e ROS_DOMAIN_ID="$r2r_domain" \
        -e ROS_LOCALHOST_ONLY=1 -e R2R_DISTRO="$r2r_distro" -e R2R_RMWS="$r2r_rmws" \
        "$r2r_image" -lc '
            set -eo pipefail
            ulimit -v 10485760
            source "/opt/ros/$R2R_DISTRO/setup.bash"
            set -u
            unset CYCLONEDDS_URI FASTRTPS_DEFAULT_PROFILES_FILE FASTDDS_DEFAULT_PROFILES_FILE
            export PATH=/r2r-cargo/bin:$PATH
            cd /r2r
            for r2r_rmw in $R2R_RMWS; do
                export RMW_IMPLEMENTATION="$r2r_rmw"
                printf "%s\n" "ROS_DISTRO=$ROS_DISTRO RMW_IMPLEMENTATION=$RMW_IMPLEMENTATION"
                cargo test --offline --locked -p r2r --lib reset_events -- --test-threads=1
                cargo test --offline --locked -p r2r --lib callback_and_grouped_timers_share_reset_event_contract -- --test-threads=1
                cargo run --offline --locked -p r2r --example timer_reset_events
            done
            cargo clippy --offline --locked -p r2r --lib --example timer_reset_events -- -D warnings
        '
}

run_ros_container humble r2r_humble_validation target/timer-reset-humble rmw_fastrtps_cpp 221
run_ros_container kilted r2r_kilted_native_feedback_validation target/timer-reset-kilted \
    'rmw_cyclonedds_cpp rmw_fastrtps_cpp' 222
