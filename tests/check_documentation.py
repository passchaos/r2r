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
    "struct.LifecycleCallbackTimer.html",
    "struct.TimeSource.html",
    "struct.Publisher.html",
    "struct.NativeMessageRef.html",
    "struct.QosEventStream.html",
    "struct.RosSleep.html",
    "struct.CallbackService.html",
    "struct.CallbackServiceOptions.html",
    "struct.CallbackTimer.html",
    "struct.CallbackTimerOptions.html",
    "enum.QosEvent.html",
    "enum.PublisherEventKind.html",
    "enum.SubscriptionEventKind.html",
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
    "struct.TypeDescriptionClient.html",
    "struct.TopicTypeHash.html",
    "struct.NodeOptions.html",
    "fn.message_type_hash.html",
    "fn.service_type_hash.html",
    "fn.action_type_hash.html",
    "fn.type_description_hash.html",
    "type_description_interfaces/srv/GetTypeDescription/struct.Response.html",
)
REQUIRED_METHODS = {
    "struct.CallbackTimer.html": ("clock_type", "is_steady", "is_valid", "cancel", "reset", "is_canceled", "is_ready", "period", "set_period", "next_call_time_ns", "time_until_next_call_ns", "time_since_last_call_ns"),
    "struct.CallbackTimerOptions.html": ("autostart", "oneshot"),
    "struct.NodeOptions.html": ("start_type_description_service",),
    "struct.LifecycleNode.html": ("create_publisher", "create_publisher_with_options", "configure", "activate",
                                "create_timer", "create_timer_with_options", "create_wall_timer", "create_wall_timer_with_options",
                                "create_wall_timer_callback", "create_wall_timer_callback_with_options", "create_timer_callback", "create_timer_callback_with_options",
                                "create_wall_timer_callback_with_info", "create_wall_timer_callback_with_info_and_options", "create_timer_callback_with_info", "create_timer_callback_with_info_and_options", "destroy_callback_timer"),
    "struct.LifecyclePublisher.html": ("publish", "publish_native", "publish_native_ref", "borrow_loaned_message", "destroy", "wait_for_subscribers", "qos_events"),
    "struct.Publisher.html": ("publish_native_ref", "qos_events"),
    "struct.PublisherUntyped.html": ("qos_events",),
    "struct.Subscription.html": ("qos_events",),
    "struct.LifecycleLoanedMessage.html": ("publish",),
    "struct.Client.html": ("configure_introspection",),
    "struct.TimeSource.html": ("enable_sim_time", "disable_sim_time", "attach_ros_clock"),
    "struct.LifecycleTimer.html": ("tick", "tick_with_info", "cancel", "reset", "destroy"),
    "struct.LifecycleCallbackTimer.html": ("clock_type", "is_steady", "is_valid", "cancel", "reset", "is_canceled", "is_ready", "period", "set_period", "time_until_next_call_ns", "time_since_last_call_ns", "next_call_time_ns", "destroy"),
    "struct.Executor.html": ("spawn",),
    "struct.ExecutorTask.html": ("handle", "cancel", "is_finished", "stop"),
    "struct.ActionClientOptions.html": ("feedback_overflow_policy",),
    "struct.ActionServerGoal.html": ("execute_batch", "succeed_batch", "abort_batch", "cancel_batch"),
    "struct.ActionServerGoalRequest.html": ("accept_batch", "accept_deferred_batch"),
    "struct.ActionServerGoalUntyped.html": ("execute_batch", "succeed_batch", "abort_batch", "cancel_batch"),
    "struct.ActionServerGoalRequestUntyped.html": ("accept_batch", "accept_deferred_batch"),
    "struct.Node.html": ("subscribe_native_callback", "subscribe_native_callback_with_options",
                         "destroy_native_callback_subscription", "destroy_qos_event_stream",
                         "start_type_description_service", "start_type_description_service_with_qos", "stop_type_description_service",
                         "type_description_service_name", "create_type_description_client", "create_type_description_client_with_qos",
                         "sleep_until_ros", "sleep_for_ros", "create_service_callback", "create_service_callback_with_options",
                         "create_service_native_callback", "create_service_native_callback_with_options", "destroy_callback_service",
                         "create_wall_timer_callback", "create_wall_timer_callback_with_options",
                         "create_timer_callback", "create_timer_callback_with_options",
                         "create_wall_timer_callback_with_info", "create_wall_timer_callback_with_info_and_options",
                         "create_timer_callback_with_info", "create_timer_callback_with_info_and_options", "destroy_callback_timer"),
    "struct.CallbackService.html": ("service_name", "get_request_subscription_actual_qos", "get_response_publisher_actual_qos", "configure_introspection"),
    "struct.CallbackServiceOptions.html": ("max_requests_per_spin",),
    "struct.RosSleep.html": ("deadline_ns", "cancel"),
    "struct.TypeDescriptionClient.html": ("service_name", "wait_for_service", "wait_for_service_with_timeout",
                                           "describe", "describe_with_timeout"),
    "struct.NativeCallbackSubscription.html": ("get_publisher_count", "topic_name", "actual_qos",
                                                "wait_for_publishers", "wait_for_publishers_with_timeout", "qos_events"),
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
        ["python3", "-m", "unittest", "discover", "-s", "tools/ros_oracles", "-p", "test_qos_events.py"],
        ["python3", "-m", "unittest", "discover", "-s", "tools/ros_oracles", "-p", "test_type_descriptions.py"],
        ["python3", "-m", "unittest", "discover", "-s", "tools/ros_oracles", "-p", "test_ros_sleep.py"],
        ["cargo", "check", "--workspace", "--all-targets", "--features", "doc-only"],
        ["cargo", "test", "-p", "r2r_common", "--lib", "--features", "doc-only"],
        ["cargo", "test", "-p", "r2r_msg_gen", "--lib", "--features", "doc-only", "message_path"],
        ["cargo", "test", "-p", "r2r", "--doc", "--features", "doc-only", "NativeMessageRef"],
        ["cargo", "test", "-p", "r2r", "--doc", "--features", "doc-only", "create_service_native_callback"],
        ["cargo", "test", "-p", "r2r", "--doc", "--features", "doc-only", "create_wall_timer_callback"],
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
