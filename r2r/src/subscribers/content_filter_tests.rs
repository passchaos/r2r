use super::*;
use crate::{
    std_msgs::msg::Int32, CallbackGroup, GroupedSubscriptionOptions,
    NativeCallbackSubscriptionOptions, Node, SubscriptionOptions,
};
use futures::{FutureExt, StreamExt};
use std::{
    sync::{Arc, Mutex as StdMutex},
    time::{Duration, Instant},
};

fn wait_for_match(node: &mut Node, publisher: &crate::Publisher<Int32>) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while publisher.get_subscription_count().unwrap() == 0 {
        assert!(Instant::now() < deadline, "subscription discovery timed out");
        node.try_spin_once(Duration::from_millis(10)).unwrap();
    }
}

fn next_message(node: &mut Node, subscription: &mut Subscription<Int32>) -> Int32 {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(message) = subscription.next().now_or_never().flatten() {
            return message;
        }
        assert!(Instant::now() < deadline, "filtered message timed out");
    }
}

#[test]
fn stream_subscription_filters_update_and_clear_without_recreation() {
    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context.clone(), "content_filter_stream", "").unwrap();
    let mut other = Node::create(context, "content_filter_stream_other", "").unwrap();
    let topic = format!("/content_filter_stream_{}", uuid::Uuid::new_v4().simple());
    let qos = QosProfile::default().keep_last(10).transient_local();
    let publisher = node.create_publisher::<Int32>(&topic, qos.clone()).unwrap();
    let above_ten = ContentFilterOptions::new("data > %0").expression_parameters(["10"]);
    // Retained samples already exist before the subscription. Native initialization must keep the
    // rejected sample out of delivery rather than discarding it after a Rust-side take.
    publisher.publish(&Int32 { data: 1 }).unwrap();
    publisher.publish(&Int32 { data: 42 }).unwrap();
    let mut subscription = match node.subscribe_with_options::<Int32>(
        &topic,
        qos,
        SubscriptionOptions::default().content_filter(above_ten.clone()),
    ) {
        Err(Error::RCL_RET_UNSUPPORTED) => {
            eprintln!("selected RMW does not support content-filtered subscriptions");
            return;
        }
        Err(error) => panic!("unexpected creation-time content filter error: {error}"),
        Ok(subscription) => subscription,
    };
    assert!(matches!(
        subscription.set_content_filter(&mut other, &above_ten),
        Err(Error::RCL_RET_SUBSCRIPTION_INVALID)
    ));
    assert!(subscription.is_content_filter_enabled(&node).unwrap());
    assert_eq!(subscription.get_content_filter(&node).unwrap(), above_ten);
    wait_for_match(&mut node, &publisher);
    assert_eq!(next_message(&mut node, &mut subscription).data, 42);

    let below_ten = ContentFilterOptions::new("data < %0").expression_parameters(["10"]);
    subscription
        .set_content_filter(&mut node, &below_ten)
        .unwrap();
    assert_eq!(subscription.get_content_filter(&node).unwrap(), below_ten);
    publisher.publish(&Int32 { data: 42 }).unwrap();
    publisher.publish(&Int32 { data: 7 }).unwrap();
    assert_eq!(next_message(&mut node, &mut subscription).data, 7);

    subscription.clear_content_filter(&mut node).unwrap();
    assert!(!subscription.is_content_filter_enabled(&node).unwrap());
    publisher.publish(&Int32 { data: 1 }).unwrap();
    assert_eq!(next_message(&mut node, &mut subscription).data, 1);
}

#[test]
fn native_callback_subscription_uses_the_same_runtime_filter_contract() {
    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context.clone(), "content_filter_native_callback", "").unwrap();
    let mut other = Node::create(context, "content_filter_native_callback_other", "").unwrap();
    let topic = format!("/content_filter_native_{}", uuid::Uuid::new_v4().simple());
    let publisher = node
        .create_publisher::<Int32>(&topic, QosProfile::default())
        .unwrap();
    let values = Arc::new(StdMutex::new(Vec::new()));
    let seen = values.clone();
    let filter = ContentFilterOptions::new("data > %0").expression_parameters(["10"]);
    let subscription = match node.subscribe_native_callback_with_options::<Int32, _>(
        &topic,
        QosProfile::default(),
        NativeCallbackSubscriptionOptions::default().content_filter(filter.clone()),
        move |message, _| {
            seen.lock().unwrap().push(message.data);
            Ok(())
        },
    ) {
        Err(Error::RCL_RET_UNSUPPORTED) => return,
        Err(error) => panic!("unexpected creation-time content filter error: {error}"),
        Ok(subscription) => subscription,
    };

    assert!(matches!(
        subscription.set_content_filter(&mut other, &filter),
        Err(Error::RCL_RET_SUBSCRIPTION_INVALID)
    ));
    assert!(subscription.is_content_filter_enabled(&node).unwrap());
    assert_eq!(subscription.get_content_filter(&node).unwrap(), filter);
    wait_for_match(&mut node, &publisher);
    publisher.publish(&Int32 { data: 1 }).unwrap();
    publisher.publish(&Int32 { data: 42 }).unwrap();

    let deadline = Instant::now() + Duration::from_secs(3);
    while values.lock().unwrap().is_empty() {
        assert!(Instant::now() < deadline, "native callback message timed out");
        node.try_spin_once(Duration::from_millis(10)).unwrap();
    }
    assert_eq!(&*values.lock().unwrap(), &[42]);
    subscription.clear_content_filter(&mut node).unwrap();
    assert!(!subscription.is_content_filter_enabled(&node).unwrap());
}

#[test]
fn grouped_subscription_filter_access_resolves_the_grouped_registry() {
    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context.clone(), "content_filter_grouped", "").unwrap();
    let mut other = Node::create(context, "content_filter_grouped_other", "").unwrap();
    let mut group = CallbackGroup::new();
    let topic = format!("/content_filter_grouped_{}", uuid::Uuid::new_v4().simple());
    let filter = ContentFilterOptions::new("data = %0").expression_parameters(["42"]);
    let subscription = match node.subscribe_callback_in_group_with_options::<Int32, _>(
        &mut group,
        &topic,
        QosProfile::default(),
        GroupedSubscriptionOptions::default().content_filter(filter.clone()),
        |_, _| Ok(()),
    ) {
        Err(Error::RCL_RET_UNSUPPORTED) => return,
        Err(error) => panic!("unexpected creation-time content filter error: {error}"),
        Ok(subscription) => subscription,
    };

    assert!(matches!(
        subscription.set_content_filter(&mut other, &filter),
        Err(Error::RCL_RET_SUBSCRIPTION_INVALID)
    ));
    assert!(subscription.is_content_filter_enabled(&node).unwrap());
    assert_eq!(subscription.get_content_filter(&node).unwrap(), filter);
    subscription.clear_content_filter(&mut node).unwrap();
    assert!(!subscription.is_content_filter_enabled(&node).unwrap());
}

#[test]
fn creation_filter_builders_preserve_copy_endpoint_options() {
    let filter = ContentFilterOptions::new("data = %0").expression_parameters(["7"]);

    let stream = SubscriptionOptions::default().channel_capacity(3);
    let stream_configuration = stream.content_filter(filter.clone());
    assert_eq!(stream.channel_capacity, 3);
    assert_eq!(stream_configuration.endpoint, stream);
    assert_eq!(stream_configuration.content_filter.as_ref(), Some(&filter));

    let callback = NativeCallbackSubscriptionOptions::default().max_messages_per_spin(2);
    let callback_configuration = callback.content_filter(filter.clone());
    assert_eq!(callback.max_messages_per_spin, 2);
    assert_eq!(callback_configuration.endpoint, callback);
    assert_eq!(callback_configuration.content_filter.as_ref(), Some(&filter));

    let grouped = GroupedSubscriptionOptions::default();
    let grouped_configuration = grouped.content_filter(filter.clone());
    assert_eq!(grouped_configuration.endpoint, grouped);
    assert_eq!(grouped_configuration.content_filter, Some(filter));
}

#[test]
fn typed_and_native_batch_streams_apply_creation_filters_before_the_first_take() {
    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "content_filter_batches", "").unwrap();
    let topic = format!("/content_filter_batches_{}", uuid::Uuid::new_v4().simple());
    let qos = QosProfile::default().keep_last(10).transient_local();
    let publisher = node.create_publisher::<Int32>(&topic, qos.clone()).unwrap();
    publisher.publish(&Int32 { data: 1 }).unwrap();
    publisher.publish(&Int32 { data: 42 }).unwrap();
    let filter = ContentFilterOptions::new("data > %0").expression_parameters(["10"]);
    let configuration = SubscriptionOptions::default().content_filter(filter.clone());
    let mut typed = match node.subscribe_batches_with_options::<Int32>(
        &topic,
        qos.clone(),
        4,
        configuration.clone(),
    ) {
        Err(Error::RCL_RET_UNSUPPORTED) => return,
        Err(error) => panic!("unexpected typed batch content filter error: {error}"),
        Ok(subscription) => subscription,
    };
    let mut native = node
        .subscribe_native_batches_with_options::<Int32>(&topic, qos, 4, configuration)
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(3);
    while publisher.get_subscription_count().unwrap() != 2 {
        assert!(Instant::now() < deadline, "batch subscription discovery timed out");
        node.try_spin_once(Duration::from_millis(10)).unwrap();
    }
    let typed_batch = loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(batch) = typed.next().now_or_never().flatten() {
            break batch;
        }
        assert!(Instant::now() < deadline, "filtered typed batch timed out");
    };
    let native_batch = loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(batch) = native.next().now_or_never().flatten() {
            break batch;
        }
        assert!(Instant::now() < deadline, "filtered native batch timed out");
    };
    assert_eq!(
        typed_batch
            .messages()
            .iter()
            .map(|message| message.data)
            .collect::<Vec<_>>(),
        [42]
    );
    assert_eq!(native_batch.len(), 1);
    assert_eq!(native_batch.message(0).unwrap().data, 42);
    assert_eq!(typed.get_content_filter(&node).unwrap(), filter);
    assert_eq!(native.get_content_filter(&node).unwrap(), filter);
}
