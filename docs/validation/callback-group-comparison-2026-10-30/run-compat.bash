#!/usr/bin/env bash
set -euo pipefail
run_native() {
    local r2r_distro="$1" r2r_image="$2" r2r_target="$3" r2r_rmws="$4"
    mkdir -p "$r2r_target"
    docker run --rm --entrypoint bash \
        -v "$PWD:/r2r:ro" -v "$PWD/$r2r_target:/target" \
        -v /home/passchaos/.cargo:/r2r-cargo:ro -v /home/passchaos/.rustup:/r2r-rustup:ro \
        -e CARGO_HOME=/r2r-cargo -e RUSTUP_HOME=/r2r-rustup -e RUSTUP_TOOLCHAIN=1.85.0 \
        -e CARGO_NET_OFFLINE=true -e CARGO_TARGET_DIR=/target/candidate-cargo \
        -e ROS_DOMAIN_ID=181 -e ROS_LOCALHOST_ONLY=1 -e R2R_DISTRO="$r2r_distro" -e R2R_RMWS="$r2r_rmws" \
        "$r2r_image" -c '
            set -e
            source "/opt/ros/$R2R_DISTRO/setup.bash"
            unset CYCLONEDDS_URI FASTRTPS_DEFAULT_PROFILES_FILE FASTDDS_DEFAULT_PROFILES_FILE
            export PATH=/r2r-cargo/bin:$PATH
            cd /r2r
            python3 tools/bench_ros/build.py --workload callback_groups --with-probe --output-dir /target
            for r2r_rmw in $R2R_RMWS; do
                python3 tools/bench_ros/callback_groups.py --rust /target/r2r-candidate --cpp /target/rclcpp-comparison \
                    --probe /target/libr2r_bench_probe.so --build-manifest /target/build-manifest.json --cpus 24,25,26,27 \
                    --cases exclusive:1:1:0,reentrant:1:4:0,exclusive:4:8:16384,reentrant:2:4:0,reentrant:4:8:16384 \
                    --iterations 64 --warmup 16 --repetitions 2 --rmw "$r2r_rmw" --output "/target/protocol-$r2r_rmw.json"
            done
            cargo clippy --offline --locked -p r2r --bench callback_groups -- -D warnings
        '
}
run_native humble r2r_humble_validation target/callback-comparison-humble rmw_fastrtps_cpp > target/callback-comparison-humble.log 2>&1
run_native kilted r2r_kilted_native_feedback_validation target/callback-comparison-kilted 'rmw_cyclonedds_cpp rmw_fastrtps_cpp' > target/callback-comparison-kilted.log 2>&1
bash docs/validation/reentrant-groups-2026-10-30/run-no-ros.bash
