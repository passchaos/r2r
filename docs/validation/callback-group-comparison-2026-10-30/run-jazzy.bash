#!/usr/bin/env bash
set -eo pipefail
source /opt/ros/jazzy/setup.bash
set -u
export CARGO_NET_OFFLINE=true
unset CYCLONEDDS_URI FASTRTPS_DEFAULT_PROFILES_FILE FASTDDS_DEFAULT_PROFILES_FILE
python3 -m unittest discover -s tools/bench_ros -p 'test_*.py'
python3 tools/bench_ros/probe/check.py --output-dir target/callback-group-probe-validation
python3 tools/bench_ros/build.py --workload callback_groups --with-probe --output-dir target/callback-group-comparison
for r2r_rmw in rmw_cyclonedds_cpp rmw_fastrtps_cpp; do
    python3 tools/bench_ros/callback_groups.py \
        --rust target/callback-group-comparison/r2r-candidate --cpp target/callback-group-comparison/rclcpp-comparison \
        --probe target/callback-group-comparison/libr2r_bench_probe.so --build-manifest target/callback-group-comparison/build-manifest.json \
        --cpus 24,25,26,27 --cases exclusive:1:1:0,reentrant:1:4:0,exclusive:4:8:16384,reentrant:2:4:0,reentrant:4:8:16384 \
        --iterations 64 --warmup 16 --repetitions 2 --rmw "$r2r_rmw" \
        --output "target/callback-group-final-$r2r_rmw.json"
done
cargo clippy --offline --locked --workspace --all-targets -- -D warnings
