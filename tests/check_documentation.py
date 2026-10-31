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
    "trait.WrappedTypesupport.html",
    "trait.WrappedServiceTypeSupport.html",
    "trait.WrappedActionTypeSupport.html",
    "trait.RosParams.html",
    "struct.ActionClient.html",
    "struct.ActionClientNativeGoalResponse.html",
    "struct.ActionNativeFeedback.html",
    "struct.ActionNativeFeedbackStream.html",
    "struct.QosEventStream.html",
    "struct.RosSleep.html",
    "struct.CallbackService.html",
    "struct.CallbackServiceOptions.html",
    "struct.CallbackTimer.html",
    "struct.CallbackTimerOptions.html",
    "struct.Timer.html",
    "struct.TimerReset.html",
    "struct.TimerResetEvents.html",
    "enum.QosEvent.html",
    "enum.PublisherEventKind.html",
    "enum.SubscriptionEventKind.html",
    "enum.ServiceIntrospectionState.html",
    "struct.MessageSerializer.html",
    "struct.Executor.html",
    "struct.ExecutorTask.html",
    "enum.ExecutorTaskError.html",
    "struct.ParallelExecutor.html",
    "struct.ParallelExecutorHandle.html",
    "struct.ParallelExecutorTask.html",
    "enum.ParallelExecutorTaskError.html",
    "struct.CallbackExecutor.html",
    "struct.CallbackExecutorHandle.html",
    "struct.CallbackExecutorTask.html",
    "enum.CallbackExecutorTaskError.html",
    "enum.CallbackWorkerError.html",
    "struct.CallbackGroup.html",
    "struct.CallbackGroupTask.html",
    "struct.ReentrantCallbackGroup.html",
    "struct.CallbackGroupId.html",
    "struct.CallbackQueueOptions.html",
    "struct.GroupedSubscription.html",
    "struct.GroupedSubscriptionOptions.html",
    "struct.GroupedService.html",
    "struct.GroupedTimer.html",
    "enum.FeedbackOverflowPolicy.html",
    "struct.ActionClientOptions.html",
    "struct.ActionServerGoal.html",
    "struct.ActionServerGoalRequest.html",
    "struct.ActionServerGoalUntyped.html",
    "struct.ActionServerGoalRequestUntyped.html",
    "struct.Node.html",
    "struct.Context.html",
    "struct.ContextShutdown.html",
    "struct.ParameterCallbackHandle.html",
    "struct.ParameterPreSetCallbackHandle.html",
    "struct.ParameterPostSetCallbackHandle.html",
    "struct.Subscription.html",
    "struct.SubscriptionOptions.html",
    "struct.SubscriptionConfiguration.html",
    "struct.ContentFilterOptions.html",
    "struct.NativeSubscriptionSample.html",
    "struct.ReusableSubscriptionSample.html",
    "struct.NativeSubscriptionBatch.html",
    "struct.SubscriptionBatch.html",
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
    "trait.WrappedTypesupport.html": ("try_get_ts", "try_create_msg", "update_from_native",
                                      "try_reinitialize_native"),
    "trait.WrappedServiceTypeSupport.html": ("try_get_ts",),
    "trait.WrappedActionTypeSupport.html": ("try_get_ts", "supports_native_feedback_delivery"),
    "trait.RosParams.html": ("register_parameter_descriptors", "set_parameters_atomically"),
    "struct.ActionClient.html": ("send_goal_request_native_feedback",
                                  "send_goal_request_native_feedback_with_options",
                                  "send_goal_request_native_feedback_with_timeout",
                                  "send_goal_request_native_feedback_with_options_and_timeout"),
    "struct.ActionNativeFeedback.html": ("goal_id", "native_message"),
    "struct.ContentFilterOptions.html": ("new", "expression_parameters"),
    "struct.SubscriptionOptions.html": ("content_filter",),
    "struct.SubscriptionConfiguration.html": ("new", "content_filter"),
    "struct.NativeSubscriptionSample.html": ("native_message",),
    "struct.ReusableSubscriptionSample.html": ("message",),
    "struct.NativeSubscriptionBatch.html": ("len", "is_empty", "capacity", "message", "info", "iter"),
    "struct.SubscriptionBatch.html": ("len", "is_empty", "capacity", "messages", "infos", "iter"),
    "struct.CallbackTimer.html": ("clock_type", "is_steady", "is_valid", "cancel", "reset", "observe_resets", "is_canceled", "is_ready", "period", "set_period", "next_call_time_ns", "time_until_next_call_ns", "time_since_last_call_ns"),
    "struct.CallbackTimerOptions.html": ("autostart", "oneshot"),
    "struct.Timer.html": ("observe_resets",),
    "struct.TimerResetEvents.html": ("total", "pending"),
    "struct.NodeOptions.html": ("start_type_description_service",),
    "struct.LifecycleNode.html": ("create_publisher", "create_publisher_with_options", "configure", "activate",
                                "create_timer", "create_timer_with_options", "create_wall_timer", "create_wall_timer_with_options",
                                "create_wall_timer_callback", "create_wall_timer_callback_with_options", "create_timer_callback", "create_timer_callback_with_options",
                                "create_wall_timer_callback_with_info", "create_wall_timer_callback_with_info_and_options", "create_timer_callback_with_info", "create_timer_callback_with_info_and_options", "destroy_callback_timer"),
    "struct.LifecyclePublisher.html": ("publish", "publish_native", "publish_native_ref", "borrow_loaned_message", "destroy", "wait_for_subscribers", "qos_events"),
    "struct.Publisher.html": ("publish_native_ref", "qos_events"),
    "struct.PublisherUntyped.html": ("qos_events",),
    "struct.Subscription.html": ("qos_events", "is_content_filter_enabled", "set_content_filter",
                                  "clear_content_filter", "get_content_filter"),
    "struct.LifecycleLoanedMessage.html": ("publish",),
    "struct.Client.html": ("configure_introspection",),
    "struct.TimeSource.html": ("enable_sim_time", "disable_sim_time", "attach_ros_clock"),
    "struct.LifecycleTimer.html": ("tick", "tick_with_info", "cancel", "reset", "observe_resets", "destroy"),
    "struct.LifecycleCallbackTimer.html": ("clock_type", "is_steady", "is_valid", "cancel", "reset", "observe_resets", "is_canceled", "is_ready", "period", "set_period", "time_until_next_call_ns", "time_since_last_call_ns", "next_call_time_ns", "destroy"),
    "struct.Executor.html": ("spawn",),
    "struct.ExecutorTask.html": ("handle", "cancel", "is_finished", "stop"),
    "struct.ParallelExecutor.html": ("new", "worker_count", "len", "is_empty", "add_node", "add_node_to_worker", "node_mut", "remove_node", "handle", "is_cancelled", "reset_cancel", "spawn"),
    "struct.ParallelExecutorHandle.html": ("cancel", "is_cancelled"),
    "struct.ParallelExecutorTask.html": ("worker_count", "handle", "cancel", "is_finished", "stop"),
    "struct.CallbackExecutor.html": ("new", "worker_count", "add_node", "node_mut", "remove_node", "len", "is_empty", "add_group", "group_mut", "node_and_group_mut", "handle", "is_cancelled", "reset_cancel", "spawn"),
    "struct.CallbackExecutorHandle.html": ("cancel", "is_cancelled"),
    "struct.CallbackExecutorTask.html": ("handle", "worker_count", "cancel", "is_finished", "stop"),
    "struct.CallbackGroup.html": ("new", "id", "len", "is_empty", "spawn_task"),
    "struct.CallbackGroupTask.html": ("cancel", "is_cancelled", "is_finished", "detach"),
    "struct.ReentrantCallbackGroup.html": ("new", "id", "concurrency", "is_empty", "spawn_task"),
    "struct.CallbackQueueOptions.html": ("capacity", "max_inputs_per_spin"),
    "struct.GroupedSubscriptionOptions.html": ("queue", "endpoint", "content_filter"),
    "struct.GroupedSubscription.html": ("topic_name", "actual_qos", "is_content_filter_enabled",
                                           "set_content_filter", "clear_content_filter", "get_content_filter",
                                           "get_publisher_count", "wait_for_publishers", "wait_for_publishers_with_timeout",
                                           "qos_events", "is_valid"),
    "struct.GroupedService.html": ("is_valid", "service_name", "get_request_subscription_actual_qos", "get_response_publisher_actual_qos", "configure_introspection"),
    "struct.GroupedTimer.html": ("is_valid", "clock_type", "is_steady", "cancel", "reset", "observe_resets", "is_canceled", "period", "next_call_time_ns", "set_period"),
    "struct.ActionClientOptions.html": ("feedback_overflow_policy",),
    "struct.ActionServerGoal.html": ("execute_batch", "succeed_batch", "abort_batch", "cancel_batch"),
    "struct.ActionServerGoalRequest.html": ("accept_batch", "accept_deferred_batch"),
    "struct.ActionServerGoalUntyped.html": ("execute_batch", "succeed_batch", "abort_batch", "cancel_batch"),
    "struct.ActionServerGoalRequestUntyped.html": ("accept_batch", "accept_deferred_batch"),
    "struct.Node.html": ("subscribe_native_callback", "subscribe_native_callback_with_options",
                         "subscribe_native_reusable", "subscribe_native_reusable_with_options",
                         "subscribe_reusable", "subscribe_reusable_with_options",
                         "subscribe_batches", "subscribe_batches_with_options",
                         "subscribe_native_batches", "subscribe_native_batches_with_options",
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
                                                "is_content_filter_enabled", "set_content_filter",
                                                "clear_content_filter", "get_content_filter",
                                                "wait_for_publishers", "wait_for_publishers_with_timeout", "qos_events"),
    "struct.NativeCallbackSubscriptionOptions.html": ("max_messages_per_spin",
                                                       "ignore_local_publications",
                                                       "unique_network_flow", "content_filter"),
}


REQUIRED_METHODS["struct.Node.html"] += (
    "declare_parameter", "declare_parameter_with_descriptor", "declare_parameters_with_descriptors",
    "set_parameter", "set_parameters", "set_parameters_atomically", "undeclare_parameter",
    "get_parameter_descriptor", "add_pre_set_parameters_callback",
    "add_on_set_parameters_callback", "add_post_set_parameters_callback",
    "subscribe_callback_in_group", "subscribe_callback_in_group_with_options",
    "subscribe_native_callback_in_group", "subscribe_native_callback_in_group_with_options", "destroy_grouped_subscription",
    "create_service_callback_in_group", "create_service_callback_in_group_with_options",
    "create_service_native_callback_in_group", "create_service_native_callback_in_group_with_options", "destroy_grouped_service",
    "create_wall_timer_callback_in_group", "create_wall_timer_callback_in_group_with_options",
    "create_timer_callback_in_group", "create_timer_callback_in_group_with_options",
    "create_wall_timer_callback_in_group_with_info", "create_wall_timer_callback_in_group_with_info_and_options",
    "create_timer_callback_in_group_with_info", "create_timer_callback_in_group_with_info_and_options",
)
REQUIRED_METHODS["struct.Context.html"] = ("install_signal_handler", "wait_for_shutdown", "shutdown", "is_valid")
REQUIRED_METHODS["struct.LifecycleNode.html"] += (
    "create_wall_timer_callback_in_group", "create_wall_timer_callback_in_group_with_options",
    "create_timer_callback_in_group", "create_timer_callback_in_group_with_options",
    "create_wall_timer_callback_in_group_with_info", "create_wall_timer_callback_in_group_with_info_and_options",
    "create_timer_callback_in_group_with_info", "create_timer_callback_in_group_with_info_and_options",
)


REQUIRED_METHODS["struct.CallbackExecutor.html"] += (
    "add_reentrant_group", "reentrant_group_mut", "node_and_reentrant_group_mut",
)
REENTRANT_TIMERS = (
    "create_wall_timer_reentrant_callback", "create_wall_timer_reentrant_callback_with_options",
    "create_timer_reentrant_callback", "create_timer_reentrant_callback_with_options",
    "create_wall_timer_reentrant_callback_with_info", "create_wall_timer_reentrant_callback_with_info_and_options",
    "create_timer_reentrant_callback_with_info", "create_timer_reentrant_callback_with_info_and_options",
)
REQUIRED_METHODS["struct.LifecycleNode.html"] += REENTRANT_TIMERS
REQUIRED_METHODS["struct.Node.html"] += REENTRANT_TIMERS + (
    "subscribe_reentrant_callback", "subscribe_reentrant_callback_with_options",
    "subscribe_native_reentrant_callback", "subscribe_native_reentrant_callback_with_options",
    "create_service_reentrant_callback", "create_service_reentrant_callback_with_options",
    "create_service_native_reentrant_callback", "create_service_native_reentrant_callback_with_options",
)


def documentation_environment():
    environment = os.environ.copy()
    for variable in ROS_ENVIRONMENT:
        environment.pop(variable, None)
    return environment


def check(environment=None):
    environment = documentation_environment() if environment is None else environment
    commands = (
        ["python3", "tools/test_refresh_bindings.py"],
        ["python3", "-m", "unittest", "discover", "-s", "tools/bench_ros", "-p", "test_callback_groups.py"],
        ["python3", "-m", "unittest", "discover", "-s", "tools/bench_ros", "-p", "test_action_feedback.py"],
        ["python3", "tools/bench_ros/native_action_feedback_stage.py", "verify", "--require-current-sources",
         "docs/validation/native-action-feedback-2026-10-30/allocation-stage.json"],
        ["python3", "tools/bench_ros/native_subscription_reuse_stage.py", "verify", "--require-current-sources",
         "docs/validation/reusable-native-subscriptions-2026-10-31/allocation-stage.json"],
        ["python3", "tools/bench_ros/typed_subscription_reuse_stage.py", "verify", "--require-current-sources",
         "docs/validation/reusable-typed-subscriptions-2026-10-31/allocation-stage.json"],
        ["python3", "tools/bench_ros/typed_subscription_native_reuse_stage.py", "verify", "--require-current-sources",
         "docs/validation/reusable-typed-subscriptions-2026-10-31/default-typed-native-reuse-stage.json"],
        ["python3", "tools/bench_ros/typed_publisher_native_reuse_stage.py", "verify", "--require-current-sources",
         "docs/validation/typed-publisher-native-reuse-2026-10-31/allocation-stage.json"],
        ["python3", "tools/bench_ros/typed_client_request_native_reuse_stage.py", "verify", "--require-current-sources",
         "docs/validation/typed-client-request-native-reuse-2026-10-31/allocation-stage.json"],
        ["python3", "tools/bench_ros/typed_service_native_reuse_stage.py", "verify", "--require-current-sources",
         "docs/validation/typed-service-native-reuse-2026-10-31/allocation-stage.json"],
        ["python3", "tools/bench_ros/typed_client_response_native_reuse_stage.py", "verify", "--require-current-sources",
         "docs/validation/typed-client-response-native-reuse-2026-10-31/allocation-stage.json"],
        ["python3", "tools/bench_ros/typed_action_goal_request_native_reuse_stage.py", "verify", "--require-current-sources",
         "docs/validation/typed-action-goal-request-native-reuse-2026-10-31/allocation-stage.json"],
        ["python3", "tools/bench_ros/typed_action_status_native_reuse_stage.py", "verify", "--require-current-sources",
         "docs/validation/typed-action-status-native-reuse-2026-11-01/allocation-stage.json"],
        ["python3", "tools/bench_ros/typed_action_result_response_native_reuse_stage.py", "verify", "--require-current-sources",
         "docs/validation/typed-action-result-response-native-reuse-2026-11-01/allocation-stage.json"],
        ["python3", "tools/bench_ros/typed_action_lifecycle_native_reuse_stage.py", "verify", "--require-current-sources",
         "docs/validation/typed-action-lifecycle-native-reuse-2026-11-01/allocation-stage.json"],
        ["python3", "tools/bench_ros/typed_action_server_request_native_reuse_stage.py", "verify", "--require-current-sources",
         "docs/validation/typed-action-server-request-native-reuse-2026-11-01/allocation-stage.json"],
        ["python3", "tools/bench_ros/typed_action_server_feedback_native_reuse_stage.py", "verify", "--require-current-sources",
         "docs/validation/typed-action-server-feedback-native-reuse-2026-11-01/allocation-stage.json"],
        ["python3", "tools/bench_ros/native_batch_reset_stage.py", "verify", "--require-current-sources",
         "docs/validation/batch-subscriptions-2026-10-31/reset-allocation-stage.json"],
        ["python3", "tools/bench_ros/native_batch_sparse_reset_stage.py", "verify", "--require-current-sources",
         "docs/validation/batch-subscriptions-2026-10-31/sparse-reset-stage.json"],
        ["python3", "docs/validation/reusable-typed-pipeline-2026-10-31/verify.py"],
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
        ["cargo", "test", "-p", "r2r", "--doc", "--features", "doc-only", "ParallelExecutor"],
        ["cargo", "test", "-p", "r2r", "--doc", "--features", "doc-only", "CallbackExecutor"],
        ["cargo", "test", "-p", "r2r", "--doc", "--features", "doc-only", "subscribe_native_callback_in_group"],
        ["cargo", "test", "-p", "r2r", "--doc", "--features", "doc-only", "ReentrantCallbackGroup"],
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
