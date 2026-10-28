#!/usr/bin/env python3
"""Verify compilation and public API documentation without a ROS environment."""

import os
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
ROS_ENVIRONMENT = (
    "AMENT_PREFIX_PATH", "CMAKE_PREFIX_PATH", "CMAKE_IDL_PACKAGES",
    "CMAKE_INCLUDE_DIRS", "IDL_PACKAGE_FILTER", "ROS_DISTRO", "LD_LIBRARY_PATH",
    "RMW_IMPLEMENTATION", "CYCLONEDDS_URI",
)
REQUIRED_DOCUMENTATION = (
    "struct.LifecycleNode.html",
    "struct.LifecyclePublisher.html",
    "struct.LifecycleLoanedMessage.html",
    "struct.TimeSource.html",
    "enum.ServiceIntrospectionState.html",
    "struct.MessageSerializer.html",
    "struct.Executor.html",
    "example_interfaces/srv/AddTwoInts/struct.Event.html",
    "example_interfaces/action/Fibonacci/SendGoal/struct.Event.html",
    "service_msgs/msg/struct.ServiceEventInfo.html",
)
REQUIRED_METHODS = {
    "struct.LifecycleNode.html": ("create_publisher", "create_publisher_with_options", "configure", "activate"),
    "struct.LifecyclePublisher.html": ("publish", "publish_native", "borrow_loaned_message", "destroy", "wait_for_subscribers"),
    "struct.LifecycleLoanedMessage.html": ("publish",),
    "struct.Client.html": ("configure_introspection",),
    "struct.TimeSource.html": ("enable_sim_time", "disable_sim_time", "attach_ros_clock"),
}


def documentation_environment():
    environment = os.environ.copy()
    for variable in ROS_ENVIRONMENT:
        environment.pop(variable, None)
    return environment


def check(environment=None):
    environment = documentation_environment() if environment is None else environment
    commands = (
        ["python3", "tools/test_refresh_bindings.py"],
        ["cargo", "check", "--workspace", "--all-targets", "--features", "doc-only"],
        ["cargo", "test", "-p", "r2r_common", "--lib", "--features", "doc-only"],
        ["cargo", "test", "-p", "r2r_msg_gen", "--lib", "--features", "doc-only", "message_path"],
        ["cargo", "doc", "-p", "r2r", "--no-deps", "--features", "doc-only"],
    )
    for command in commands:
        subprocess.run(command, cwd=ROOT, env=environment, check=True)
    target = Path(environment.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not target.is_absolute():
        target = ROOT / target
    documentation = target / "doc" / "r2r"
    for name in REQUIRED_DOCUMENTATION:
        if not (documentation / name).is_file():
            raise RuntimeError(f"missing public API documentation: {name}")
        print(f"PASS public documentation: {name}")
    for name, methods in REQUIRED_METHODS.items():
        page = (documentation / name).read_text()
        for method in methods:
            if f'id="method.{method}"' not in page:
                raise RuntimeError(f"missing public documentation method: {name}::{method}")
            print(f"PASS documented method: {name}::{method}")
    print(f"{len(REQUIRED_DOCUMENTATION)} public documentation artifacts verified without ROS")


if __name__ == "__main__":
    check()
