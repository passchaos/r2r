use super::*;
use crate::{std_msgs::msg::UInt64, FutureExecutionOptions, QosEvent, QosEventStream};
use futures::{FutureExt, StreamExt};
use std::time::Instant;

fn node(name: &str) -> Node {
    Node::create(Context::create().unwrap(), name, "").unwrap()
}
fn topic(name: &str) -> String {
    format!("/{name}_{}", uuid::Uuid::new_v4().simple())
}

fn incompatible_pair(node: &mut Node, topic: &str) -> (Publisher<UInt64>, Subscription<UInt64>) {
    let publisher = node
        .create_publisher::<UInt64>(topic, QosProfile::default().best_effort())
        .unwrap();
    let subscription = node
        .subscribe::<UInt64>(topic, QosProfile::default().reliable())
        .unwrap();
    (publisher, subscription)
}

fn next_event(node: &mut Node, stream: &mut QosEventStream) -> QosEvent {
    node.spin_until_future_complete_with_options(
        async { stream.next().await.ok_or(Error::RCL_RET_ERROR)? },
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap()
}

mod execution;
mod ownership;
mod readiness;
