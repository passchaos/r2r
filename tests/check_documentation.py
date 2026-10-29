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
    "struct.LifecycleTimer.html",
    "struct.TimeSource.html",
    "struct.Publisher.html",
    "struct.NativeMessageRef.html",
    "enum.ServiceIntrospectionState.html",
    "struct.MessageSerializer.html",
    "struct.Executor.html",
    "struct.ExecutorTask.html",
    "enum.ExecutorTaskError.html",
    "enum.FeedbackOverflowPolicy.html",
    "struct.ActionClientOptions.html",
    "struct.ActionServerGoal.html",
    "struct.ActionServerGoalRequest.html",
    "struct.ActionServerGoalUntyped.html",
    "struct.ActionServerGoalRequestUntyped.html",
    "struct.Node.html",
    "struct.NativeCallbackSubscription.html",
    "struct.NativeCallbackSubscriptionOptions.html",
    "example_interfaces/srv/AddTwoInts/struct.Event.html",
    "example_interfaces/action/Fibonacci/SendGoal/struct.Event.html",
    "service_msgs/msg/struct.ServiceEventInfo.html",
)
REQUIRED_METHODS = {
    "struct.LifecycleNode.html": ("create_publisher", "create_publisher_with_options", "configure", "activate",
                                "create_timer", "create_timer_with_options", "create_wall_timer", "create_wall_timer_with_options"),
    "struct.LifecyclePublisher.html": ("publish", "publish_native", "publish_native_ref", "borrow_loaned_message", "destroy", "wait_for_subscribers"),
    "struct.Publisher.html": ("publish_native_ref",),
    "struct.LifecycleLoanedMessage.html": ("publish",),
    "struct.Client.html": ("configure_introspection",),
    "struct.TimeSource.html": ("enable_sim_time", "disable_sim_time", "attach_ros_clock"),
    "struct.LifecycleTimer.html": ("tick", "tick_with_info", "cancel", "reset", "destroy"),
    "struct.Executor.html": ("spawn",),
    "struct.ExecutorTask.html": ("handle", "cancel", "is_finished", "stop"),
    "struct.ActionClientOptions.html": ("feedback_overflow_policy",),
    "struct.ActionServerGoal.html": ("execute_batch", "succeed_batch", "abort_batch", "cancel_batch"),
    "struct.ActionServerGoalRequest.html": ("accept_batch", "accept_deferred_batch"),
    "struct.ActionServerGoalUntyped.html": ("execute_batch", "succeed_batch", "abort_batch", "cancel_batch"),
    "struct.ActionServerGoalRequestUntyped.html": ("accept_batch", "accept_deferred_batch"),
    "struct.Node.html": ("subscribe_native_callback", "subscribe_native_callback_with_options",
                         "destroy_native_callback_subscription"),
    "struct.NativeCallbackSubscription.html": ("get_publisher_count", "topic_name", "actual_qos",
                                                "wait_for_publishers", "wait_for_publishers_with_timeout"),
    "struct.NativeCallbackSubscriptionOptions.html": ("max_messages_per_spin",
                                                       "ignore_local_publications",
                                                       "unique_network_flow"),
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
        ["python3", "-m", "unittest", "discover", "-s", "tools/bench_ros", "-p", "test_pubsub_pipeline.py"],
        ["cargo", "check", "--workspace", "--all-targets", "--features", "doc-only"],
        ["cargo", "test", "-p", "r2r_common", "--lib", "--features", "doc-only"],
        ["cargo", "test", "-p", "r2r_msg_gen", "--lib", "--features", "doc-only", "message_path"],
        ["cargo", "test", "-p", "r2r", "--doc", "--features", "doc-only", "NativeMessageRef"],
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
