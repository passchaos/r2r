#!/usr/bin/env bash
set -euo pipefail
run_ros_container() {
    local r2r_distro="$1" r2r_image="$2" r2r_target="$3" r2r_rmws="$4"
    mkdir -p "$r2r_target"
    docker run --rm --entrypoint bash \
        -v "$PWD:/r2r:ro" -v "$PWD/$r2r_target:/target" \
        -v /home/passchaos/.cargo:/r2r-cargo:ro -v /home/passchaos/.rustup:/r2r-rustup:ro \
        -e CARGO_HOME=/r2r-cargo -e RUSTUP_HOME=/r2r-rustup -e RUSTUP_TOOLCHAIN=1.85.0 \
        -e CARGO_TARGET_DIR=/target -e ROS_LOG_DIR=/tmp/r2r-availability-logs \
        -e ROS_DOMAIN_ID=169 -e ROS_LOCALHOST_ONLY=1 -e R2R_DISTRO="$r2r_distro" -e R2R_RMWS="$r2r_rmws" \
        "$r2r_image" -c '
            set -e
            source "/opt/ros/$R2R_DISTRO/setup.bash"
            unset CYCLONEDDS_URI FASTRTPS_DEFAULT_PROFILES_FILE FASTDDS_DEFAULT_PROFILES_FILE
            export PATH=/r2r-cargo/bin:$PATH
            cd /r2r
            rustc --version
            cargo --version
            for r2r_rmw in $R2R_RMWS; do
                export RMW_IMPLEMENTATION="$r2r_rmw"
                printf "%s\n" "ROS_DISTRO=$ROS_DISTRO RMW_IMPLEMENTATION=$RMW_IMPLEMENTATION"
                cargo test --offline --locked -p r2r --lib --release availability -- --test-threads=1
                cargo test --offline --locked -p r2r --lib --release observer_notifications -- --test-threads=1
                cargo test --offline --locked -p r2r --lib --release executor -- --test-threads=1
                cargo test --offline --locked -p r2r --lib --release callback_service -- --test-threads=1
                cargo run --offline --locked --release -p r2r --example tokio_executor
            done
            cargo clippy --offline --locked --workspace --all-targets -- -D warnings
        '
}
run_ros_container humble r2r_humble_validation target/compat-humble-native-ref rmw_fastrtps_cpp > target/availability-final-humble.log 2>&1
run_ros_container kilted r2r_kilted_native_feedback_validation target/compat-kilted-native-ref 'rmw_cyclonedds_cpp rmw_fastrtps_cpp' > target/availability-final-kilted.log 2>&1
mkdir -p target/action-goal-batch-no-ros
docker run --rm --entrypoint bash \
    -v "$PWD:/r2r:ro" -v "$PWD/target/action-goal-batch-no-ros:/target" \
    -v /home/passchaos/.cargo:/r2r-cargo:ro -v /home/passchaos/.rustup:/r2r-rustup:ro \
    -e CARGO_HOME=/r2r-cargo -e RUSTUP_HOME=/r2r-rustup -e RUSTUP_TOOLCHAIN=1.85.0 \
    -e CARGO_TARGET_DIR=/target r2r_no_ros_validation -c '
        set -e
        test ! -d /opt/ros
        export PATH=/r2r-cargo/bin:$PATH
        cd /r2r
        rustc --version
        python3 tests/check_documentation.py
    ' > target/availability-final-no-ros.log 2>&1
