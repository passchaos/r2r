use super::*;
use crate::{
    example_interfaces::{action::Fibonacci, srv::AddTwoInts},
    std_msgs::msg::UInt64MultiArray,
    GoalStatus,
};

#[test]
fn subscriptions_timers_clients_and_services_use_disjoint_cross_node_spans() {
    let context = Context::create().unwrap();
    let mut first = node(&context, "executor_spans_first");
    let mut middle = node(&context, "executor_spans_middle");
    let mut last = node(&context, "executor_spans_last");
    let topic = format!("/executor_spans_{}", uuid::Uuid::new_v4().simple());
    let service_name = format!("{topic}/sum");
    let decoy = format!("{topic}/decoy");
    let _decoy_subscription = first
        .subscribe::<UInt64MultiArray>(&decoy, QosProfile::default())
        .unwrap();
    let _decoy_timer = first.create_wall_timer(Duration::from_secs(30)).unwrap();
    let _decoy_client = first
        .create_client::<AddTwoInts::Service>(&decoy, QosProfile::services_default())
        .unwrap();
    let _decoy_service = first
        .create_service::<AddTwoInts::Service>(&decoy, QosProfile::services_default())
        .unwrap();
    let publisher = first
        .create_publisher::<UInt64MultiArray>(&topic, QosProfile::default())
        .unwrap();
    let mut received = middle
        .subscribe::<UInt64MultiArray>(&topic, QosProfile::default())
        .unwrap();
    let mut timer = middle.create_wall_timer(Duration::from_millis(5)).unwrap();
    let mut service = middle
        .create_service::<AddTwoInts::Service>(&service_name, QosProfile::services_default())
        .unwrap();
    let client = last
        .create_client::<AddTwoInts::Service>(&service_name, QosProfile::services_default())
        .unwrap();
    let _last_subscription = last
        .subscribe::<UInt64MultiArray>(&decoy, QosProfile::default())
        .unwrap();
    let ready = client.wait_for_service().unwrap();
    let matched = publisher.wait_for_subscribers().unwrap();
    let mut executor = Executor::new(context);
    executor.add_node(first).unwrap();
    executor.add_node(middle).unwrap();
    executor.add_node(last).unwrap();
    let data: Vec<_> = (0..4096).collect();
    let server = async {
        let request = service.next().await.ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        assert_eq!(request.message.a, 11);
        let sum = request.message.a + request.message.b;
        request.respond(AddTwoInts::Response { sum })
    };
    let application = async {
        futures::try_join!(ready, matched)?;
        timer.tick().await?;
        publisher.publish(&UInt64MultiArray {
            data: data.clone(),
            ..Default::default()
        })?;
        let response = client
            .request(&AddTwoInts::Request { a: 11, b: -4 })?
            .await?;
        assert_eq!(response.sum, 7);
        let message = received
            .next()
            .await
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
        assert_eq!(message.data, data);
        Ok(())
    };
    executor
        .spin_until_future_complete_with_options(
            async {
                futures::try_join!(server, application)?;
                Ok(())
            },
            Duration::from_secs(3),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
}

#[test]
fn action_entities_follow_all_ordinary_nodes_and_complete_success_and_cancel() {
    for cancel in [false, true] {
        let context = Context::create().unwrap();
        let mut client_node = node(&context, "executor_action_client");
        let mut server_node = node(&context, "executor_action_server");
        let mut last = node(&context, "executor_action_plain_last");
        let name = format!("/executor_action_{}", uuid::Uuid::new_v4().simple());
        let client = client_node
            .create_action_client::<Fibonacci::Action>(&name)
            .unwrap();
        let mut requests = server_node
            .create_action_server::<Fibonacci::Action>(&name)
            .unwrap();
        let _ordinary_client = last
            .create_client::<AddTwoInts::Service>(
                &format!("{name}/plain"),
                QosProfile::services_default(),
            )
            .unwrap();
        let _ordinary_subscription = last
            .subscribe::<UInt64MultiArray>(&format!("{name}/plain"), QosProfile::default())
            .unwrap();
        let _ordinary_timer = last.create_wall_timer(Duration::from_secs(30)).unwrap();
        let ready = Node::is_available(&client).unwrap();
        let mut executor = Executor::new(context);
        executor.add_node(client_node).unwrap();
        executor.add_node(server_node).unwrap();
        executor.add_node(last).unwrap();
        let (cancel_acknowledged, cancel_observed) = oneshot::channel();
        let server = async {
            let request = requests
                .next()
                .await
                .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
            assert_eq!(request.goal.order, 5);
            let (mut goal, mut cancellations) = request.accept()?;
            goal.publish_feedback(Fibonacci::Feedback {
                sequence: vec![0, 1, 1, 2],
            })?;
            if cancel {
                let request = cancellations
                    .next()
                    .await
                    .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
                request.accept();
                cancel_observed
                    .await
                    .map_err(|_| Error::RCL_RET_ACTION_CLIENT_INVALID)?;
                goal.cancel(Fibonacci::Result {
                    sequence: vec![0, 1, 1, 2, 3],
                })?;
            } else {
                goal.succeed(Fibonacci::Result {
                    sequence: vec![0, 1, 1, 2, 3],
                })?;
            }
            Ok(())
        };
        let application = async {
            ready.await?;
            let (goal, result, mut feedback) = client
                .send_goal_request(Fibonacci::Goal { order: 5 })?
                .await?;
            let observed = feedback
                .next()
                .await
                .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
            assert_eq!(observed.sequence, [0, 1, 1, 2]);
            if cancel {
                goal.cancel()?.await?;
                let _ = cancel_acknowledged.send(());
            }
            let (status, result) = result.await?;
            assert_eq!(
                status,
                if cancel {
                    GoalStatus::Canceled
                } else {
                    GoalStatus::Succeeded
                }
            );
            assert_eq!(result.sequence, [0, 1, 1, 2, 3]);
            Ok(())
        };
        executor
            .spin_until_future_complete_with_options(
                async {
                    futures::try_join!(server, application)?;
                    Ok(())
                },
                Duration::from_secs(3),
                FutureExecutionOptions::wake_driven(),
            )
            .unwrap();
    }
}
