use futures::{stream, StreamExt, TryStreamExt};
use r2r::{
    example_interfaces::srv::AddTwoInts, std_msgs::msg::UInt64MultiArray, Error, Executor,
    FutureExecutionOptions, Node, QosProfile,
};
use std::time::Duration;

fn main() -> r2r::Result<()> {
    let context = r2r::Context::create()?;
    let mut sensor = Node::create(context.clone(), "pipeline_sensor", "")?;
    let mut compute = Node::create(context.clone(), "pipeline_compute", "")?;
    let mut application = Node::create(context.clone(), "pipeline_application", "")?;
    let publisher =
        sensor.create_publisher::<UInt64MultiArray>("pipeline_samples", QosProfile::default())?;
    let mut timer = sensor.create_wall_timer(Duration::from_millis(5))?;
    let mut samples =
        application.subscribe::<UInt64MultiArray>("pipeline_samples", QosProfile::default())?;
    let qos = QosProfile::services_default().keep_last(16);
    let mut service =
        compute.create_service::<AddTwoInts::Service>("pipeline_compute_sum", qos.clone())?;
    let client = application.create_client::<AddTwoInts::Service>("pipeline_compute_sum", qos)?;
    let ready = client.wait_for_service()?;
    let matched = publisher.wait_for_subscribers()?;
    let mut executor = Executor::new(context);
    let sensor_id = executor.add_node(sensor)?;
    let compute_id = executor.add_node(compute)?;
    let application_id = executor.add_node(application)?;
    let server = async {
        for _ in 0..64 {
            let request = service.next().await.ok_or(Error::RCL_RET_SERVICE_INVALID)?;
            let sum = request.message.a + request.message.b;
            request.respond(AddTwoInts::Response { sum })?;
        }
        Ok(())
    };
    let work = async {
        futures::try_join!(ready, matched)?;
        timer.tick().await?;
        publisher.publish(&UInt64MultiArray {
            data: (0..8192).collect(),
            ..Default::default()
        })?;
        let message = samples
            .next()
            .await
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
        assert_eq!(message.data, (0..8192).collect::<Vec<_>>());
        let mut responses: Vec<_> = stream::iter(0..64_i64)
            .map(|index| {
                let response = client.request(&AddTwoInts::Request { a: index, b: 5 });
                async move { Ok::<_, Error>((index, response?.await?.sum)) }
            })
            .buffer_unordered(16)
            .try_collect()
            .await?;
        responses.sort_unstable();
        for (index, sum) in responses {
            assert_eq!(sum, index + 5);
        }
        Ok(())
    };
    executor.spin_until_future_complete_with_options(
        async {
            futures::try_join!(server, work)?;
            Ok(())
        },
        Duration::from_secs(5),
        FutureExecutionOptions::wake_driven(),
    )?;
    timer.cancel(executor.node_mut(sensor_id)?)?;
    executor.node_mut(sensor_id)?.destroy_timer(timer);
    executor.node_mut(sensor_id)?.destroy_publisher(publisher);
    executor
        .node_mut(application_id)?
        .destroy_subscription(samples);
    drop(executor.remove_node(compute_id)?);
    drop(executor.remove_node(sensor_id)?);
    drop(executor.remove_node(application_id)?);
    println!("three nodes validated 8192 samples and 64 requests with at most 16 in flight");
    Ok(())
}
