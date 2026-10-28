use futures::{executor::block_on, stream, StreamExt, TryStreamExt};
use r2r::{
    example_interfaces::srv::AddTwoInts, std_msgs::msg::UInt64MultiArray, Error, Executor,
    FutureExecutionOptions, Node, QosProfile,
};
use std::{thread, time::Duration};

fn main() -> r2r::Result<()> {
    let context = r2r::Context::create()?;
    let mut server_node = Node::create(context.clone(), "controlled_server", "")?;
    let mut application_node = Node::create(context.clone(), "controlled_application", "")?;
    let mut service = server_node.create_service::<AddTwoInts::Service>(
        "controlled_sum",
        QosProfile::services_default().keep_last(16),
    )?;
    let client = application_node.create_client::<AddTwoInts::Service>(
        "controlled_sum",
        QosProfile::services_default().keep_last(16),
    )?;
    let publisher = server_node
        .create_publisher::<UInt64MultiArray>("controlled_data", QosProfile::default())?;
    let mut received =
        application_node.subscribe::<UInt64MultiArray>("controlled_data", QosProfile::default())?;
    let ready = client.wait_for_service()?;
    let matched = publisher.wait_for_subscribers()?;
    let mut executor = Executor::new(context.clone());
    let server_id = executor.add_node(server_node)?;
    let application_id = executor.add_node(application_node)?;
    let control = executor.handle();
    let worker = thread::spawn(move || {
        executor.spin()?;
        Ok::<_, Error>(executor)
    });
    let serving = async {
        for _ in 0..32 {
            let request = service.next().await.ok_or(Error::RCL_RET_SERVICE_INVALID)?;
            let sum = request.message.a + request.message.b;
            request.respond(AddTwoInts::Response { sum })?;
        }
        Ok(())
    };
    let application = async {
        futures::try_join!(ready, matched)?;
        publisher.publish(&UInt64MultiArray {
            data: (0..8192).collect(),
            ..Default::default()
        })?;
        let observed = received
            .next()
            .await
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
        assert_eq!(observed.data, (0..8192).collect::<Vec<_>>());
        let values: Vec<_> = stream::iter(0..32_i64)
            .map(|index| {
                let response = client.request(&AddTwoInts::Request { a: index, b: 5 });
                async move {
                    let response = response?.await?;
                    assert_eq!(response.sum, index + 5);
                    Ok::<_, Error>(index)
                }
            })
            .buffer_unordered(16)
            .try_collect()
            .await?;
        assert_eq!(values.len(), 32);
        Ok(())
    };
    let result = block_on(async {
        futures::try_join!(serving, application)?;
        Ok::<_, Error>(())
    });
    control.cancel()?;
    let mut executor = worker.join().expect("ROS worker panicked")?;
    result?;
    assert!(context.is_valid());
    executor.reset_cancel();
    let server = async {
        let request = service.next().await.ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        let sum = request.message.a + request.message.b;
        request.respond(AddTwoInts::Response { sum })
    };
    let request = client.request(&AddTwoInts::Request { a: 20, b: 22 })?;
    executor.spin_until_future_complete_with_options(
        async {
            let (_, response) = futures::try_join!(server, request)?;
            assert_eq!(response.sum, 42);
            Ok(())
        },
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )?;
    executor.node_mut(server_id)?.destroy_publisher(publisher);
    executor
        .node_mut(application_id)?
        .destroy_subscription(received);
    drop(service);
    drop(client);
    drop(executor.remove_node(server_id)?);
    drop(executor.remove_node(application_id)?);
    println!(
        "background executor validated 8192 values and 32 requests, stopped, and reused its nodes"
    );
    Ok(())
}
