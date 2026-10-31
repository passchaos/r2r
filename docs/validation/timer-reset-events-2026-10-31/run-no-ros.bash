#!/usr/bin/env bash
set -euo pipefail
ulimit -v 10485760
mkdir -p target/timer-reset-no-ros
docker run --rm --cpuset-cpus 0 --memory 10g --memory-swap 10g --entrypoint bash \
    -v "$PWD:/r2r:ro" -v "$PWD/target/timer-reset-no-ros:/target" \
    -v /home/passchaos/.cargo:/r2r-cargo:ro -v /home/passchaos/.rustup:/r2r-rustup:ro \
    -e CARGO_HOME=/r2r-cargo -e RUSTUP_HOME=/r2r-rustup -e RUSTUP_TOOLCHAIN=1.85.0 \
    -e CARGO_TARGET_DIR=/target -e CARGO_BUILD_JOBS=1 -e RAYON_NUM_THREADS=1 \
    r2r_no_ros_validation -lc '
        set -euo pipefail
        ulimit -v 10485760
        test ! -d /opt/ros
        export PATH=/r2r-cargo/bin:$PATH
        cd /r2r
        python3 tests/check_documentation.py
    '
