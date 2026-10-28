//! Drive lifecycle publication through native change_state services and one executor.

use std::time::Duration;

use futures::StreamExt;
use r2r::{
    lifecycle_msgs::{msg::Transition, srv::ChangeState},
    std_msgs::msg::Int32,
    Context, Error, Executor, FutureExecutionOptions, LifecycleNode, NativeMsg, Node, QosProfile,
};

fn main() -> r2r::Result<()> {
    let context = Context::create()?;
    let mut managed = LifecycleNode::create(context.clone(), "managed_publication", "")?;
    let mut observer = Node::create(context.clone(), "managed_publication_observer", "")?;
    let publisher = managed.create_publisher::<Int32>("managed_values", QosProfile::default())?;
    let mut values = observer.subscribe::<Int32>("managed_values", QosProfile::default())?;
    let client = observer.create_client::<ChangeState::Service>(
        "/managed_publication/change_state",
        QosProfile::services_default(),
    )?;
    let matched = publisher.wait_for_subscribers()?;
    let ready = client.wait_for_service()?;
    let mut executor = Executor::new(context);
    let managed_id = executor.add_node(managed.into_node())?;
    executor.add_node(observer)?;
    let execution = FutureExecutionOptions::wake_driven();
    executor.spin_until_future_complete_with_options(
        async {
            futures::try_join!(matched, ready)?;
            Ok(())
        },
        Duration::from_secs(5),
        execution,
    )?;

    let loaning = publisher.can_loan_messages()?;
    let mut native = NativeMsg::<Int32>::try_new()?;
    for (index, command) in [
        None,
        Some(Transition::TRANSITION_CONFIGURE as u8),
        Some(Transition::TRANSITION_ACTIVATE as u8),
        Some(Transition::TRANSITION_DEACTIVATE as u8),
        Some(Transition::TRANSITION_INACTIVE_SHUTDOWN as u8),
    ]
    .into_iter()
    .enumerate()
    {
        if let Some(id) = command {
            let response = client.request(&ChangeState::Request {
                transition: Transition {
                    id,
                    label: Default::default(),
                },
            })?;
            let response = executor.spin_until_future_complete_with_options(
                response,
                Duration::from_secs(5),
                execution,
            )?;
            assert!(response.success, "lifecycle command {id} failed");
        }
        let active = publisher.is_activated();
        let first = index as i32 * 10;
        publisher.clone().publish(&Int32 { data: first })?;
        native.data = first + 1;
        publisher.publish_native(&mut native)?;
        let mut expected = vec![first, first + 1];
        if loaning {
            let mut loan = publisher.borrow_loaned_message(executor.node_mut(managed_id)?)?;
            loan.data = first + 2;
            loan.publish()?;
            expected.push(first + 2);
        }
        if !active {
            expected.clear();
        }
        executor.spin_until_future_complete_with_options(
            async {
                for expected in expected {
                    let value = values
                        .next()
                        .await
                        .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
                    assert_eq!(value.data, expected);
                }
                match publisher.wait_for_all_acked_async()?.await {
                    Ok(()) | Err(Error::RCL_RET_UNSUPPORTED) => {}
                    Err(error) => return Err(error),
                }
                Ok(())
            },
            Duration::from_secs(5),
            execution,
        )?;
        let extra = executor.spin_until_future_complete_with_options(
            async {
                values
                    .next()
                    .await
                    .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)
            },
            Duration::from_millis(100),
            execution,
        );
        assert!(
            matches!(extra, Err(Error::RCL_RET_TIMEOUT)),
            "unexpected publication in phase {index}: {extra:?}"
        );
        println!("phase {index}: active={active}, typed/native publications verified, middleware loans={loaning}");
    }
    publisher.destroy(executor.node_mut(managed_id)?);
    Ok(())
}
