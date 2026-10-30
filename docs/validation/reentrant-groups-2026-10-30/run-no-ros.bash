#!/usr/bin/env bash
set -euo pipefail
mkdir -p target/action-goal-batch-no-ros
docker run --rm --entrypoint bash \
    -v "$PWD:/r2r:ro" -v "$PWD/target/action-goal-batch-no-ros:/target" \
    -v /home/passchaos/.cargo:/r2r-cargo:ro -v /home/passchaos/.rustup:/r2r-rustup:ro \
    -e CARGO_HOME=/r2r-cargo -e RUSTUP_HOME=/r2r-rustup -e RUSTUP_TOOLCHAIN=1.85.0 \
    -e CARGO_TARGET_DIR=/target r2r_no_ros_validation -c '
        set -e
        test ! -d /opt/ros
        export PATH=/r2r-cargo/bin:$PATH CARGO_NET_OFFLINE=true
        cd /r2r
        rustc --version
        python3 tests/check_documentation.py
    ' > target/reentrant-group-final-no-ros.log 2>&1
