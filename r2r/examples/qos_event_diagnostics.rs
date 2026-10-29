//! Await native QoS diagnostics on the same executor used by application futures.
use futures::StreamExt;
use r2r::{
    std_msgs::msg::UInt64, Error, FutureExecutionOptions, PublisherEventKind, QosEvent, QosProfile,
    SubscriptionEventKind,
};
use std::time::Duration;

fn main() -> r2r::Result<()> {
    let arguments: Vec<_> = std::env::args().collect();
    let external = arguments.get(1).is_some_and(|arg| arg == "--external");
    let prefix = arguments
        .get(2)
        .cloned()
        .unwrap_or_else(|| format!("/qos_diagnostics_{}", r2r::uuid::Uuid::new_v4().simple()));
    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create(context, "qos_event_diagnostics", "")?;
    let outgoing = format!("{prefix}/rust_offered");
    let incoming = format!("{prefix}/rust_requested");
    let publisher =
        node.create_publisher::<UInt64>(&outgoing, QosProfile::default().best_effort())?;
    let mut offered = publisher.qos_events(&mut node, PublisherEventKind::IncompatibleQos)?;
    let subscription = node.subscribe::<UInt64>(&incoming, QosProfile::default().reliable())?;
    let mut requested =
        subscription.qos_events(&mut node, SubscriptionEventKind::IncompatibleQos)?;
    let _peer = if external {
        None
    } else {
        Some((
            node.subscribe::<UInt64>(&outgoing, QosProfile::default().reliable())?,
            node.create_publisher::<UInt64>(&incoming, QosProfile::default().best_effort())?,
        ))
    };
    let (offered, requested) = node.spin_until_future_complete_with_options(
        async {
            let (a, b) = futures::future::join(offered.next(), requested.next()).await;
            Ok((
                a.ok_or(Error::RCL_RET_PUBLISHER_INVALID)??,
                b.ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)??,
            ))
        },
        Duration::from_secs(10),
        FutureExecutionOptions::wake_driven(),
    )?;
    assert!(
        matches!(offered, QosEvent::OfferedIncompatibleQos(status) if status.total_count == 1 && status.total_count_change == 1 && status.last_policy_kind == r2r::QosPolicyKind::Reliability)
    );
    assert!(
        matches!(requested, QosEvent::RequestedIncompatibleQos(status) if status.total_count == 1 && status.total_count_change == 1 && status.last_policy_kind == r2r::QosPolicyKind::Reliability)
    );
    println!("{{\"implementation\":\"r2r\",\"offered_count\":1,\"requested_count\":1,\"policy\":\"reliability\",\"execution\":\"native_wait\"}}");
    if external {
        use std::io::Write;
        std::io::stdout().flush().unwrap();
        let mut finish = String::new();
        std::io::stdin().read_line(&mut finish).unwrap();
        assert_eq!(finish.trim(), "finish");
    }
    Ok(())
}
