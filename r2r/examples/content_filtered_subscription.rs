//! Applies, updates and clears one middleware-side content filter.

use futures::{FutureExt, StreamExt};
use r2r::{std_msgs::msg::Int32, ContentFilterOptions, QosProfile, SubscriptionOptions};
use std::time::{Duration, Instant};

fn next_message(
    node: &mut r2r::Node, subscription: &mut r2r::Subscription<Int32>,
) -> r2r::Result<Int32> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        node.try_spin_once(Duration::from_millis(10))?;
        if let Some(message) = subscription.next().now_or_never().flatten() {
            return Ok(message);
        }
        if Instant::now() >= deadline {
            return Err(r2r::Error::RCL_RET_TIMEOUT);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create(context, "content_filtered_subscription", "")?;
    let publisher = node.create_publisher::<Int32>("filtered_numbers", QosProfile::default())?;
    let above_ten = ContentFilterOptions::new("data > %0").expression_parameters(["10"]);
    let mut subscription = match node.subscribe_with_options::<Int32>(
        "filtered_numbers",
        QosProfile::default(),
        SubscriptionOptions::default().content_filter(above_ten),
    ) {
        Err(r2r::Error::RCL_RET_UNSUPPORTED) => {
            println!("the selected RMW does not support content-filtered subscriptions");
            return Ok(());
        }
        result => result?,
    };

    let deadline = Instant::now() + Duration::from_secs(3);
    while publisher.get_subscription_count()? == 0 {
        if Instant::now() >= deadline {
            return Err(r2r::Error::RCL_RET_TIMEOUT.into());
        }
        node.try_spin_once(Duration::from_millis(10))?;
    }

    publisher.publish(&Int32 { data: 1 })?;
    publisher.publish(&Int32 { data: 42 })?;
    assert_eq!(next_message(&mut node, &mut subscription)?.data, 42);

    let below_ten = ContentFilterOptions::new("data < %0").expression_parameters(["10"]);
    subscription.set_content_filter(&mut node, &below_ten)?;
    publisher.publish(&Int32 { data: 42 })?;
    publisher.publish(&Int32 { data: 7 })?;
    assert_eq!(next_message(&mut node, &mut subscription)?.data, 7);

    subscription.clear_content_filter(&mut node)?;
    publisher.publish(&Int32 { data: 1 })?;
    assert_eq!(next_message(&mut node, &mut subscription)?.data, 1);
    println!("content filter applied, updated and cleared without recreating the subscription");
    Ok(())
}
