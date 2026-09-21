#!/bin/bash

. "$HOME/.cargo/env"

# Resolve and install the repository-pinned toolchain before sourcing ROS.
rustup show active-toolchain >/dev/null

if [ -e "/opt/ros/lyrical/setup.bash" ]; then
    source "/opt/ros/lyrical/setup.bash"
elif [ -e "/opt/ros/jazzy/setup.bash" ]; then
    source "/opt/ros/jazzy/setup.bash"
elif [ -e "/opt/ros/iron/setup.bash" ]; then
    source "/opt/ros/iron/setup.bash"
elif [ -e "/opt/ros/humble/setup.bash" ]; then
    source "/opt/ros/humble/setup.bash"
elif [ -e "/opt/ros/galactic/setup.bash" ]; then
    source "/opt/ros/galactic/setup.bash"
elif [ -e "/opt/ros/foxy/setup.bash" ]; then
    source "/opt/ros/foxy/setup.bash"
fi

cd /r2r/

"$@"
