#!/bin/bash
set -e

. "$HOME/.cargo/env"
cd /r2r/

# Resolve and install the repository-pinned toolchain before sourcing ROS.
rustup show active-toolchain >/dev/null

if [ -e "/opt/ros/rolling/setup.bash" ]; then
    source "/opt/ros/rolling/setup.bash"
elif [ -e "/opt/ros/lyrical/setup.bash" ]; then
    source "/opt/ros/lyrical/setup.bash"
elif [ -e "/opt/ros/kilted/setup.bash" ]; then
    source "/opt/ros/kilted/setup.bash"
elif [ -e "/opt/ros/jazzy/setup.bash" ]; then
    source "/opt/ros/jazzy/setup.bash"
elif [ -e "/opt/ros/iron/setup.bash" ]; then
    source "/opt/ros/iron/setup.bash"
elif [ -e "/opt/ros/humble/setup.bash" ]; then
    source "/opt/ros/humble/setup.bash"
fi

"$@"
