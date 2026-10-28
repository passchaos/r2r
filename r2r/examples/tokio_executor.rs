//! TCP requests composed with ROS services and pub/sub on current-thread Tokio.

use std::time::Duration;

use futures::StreamExt;
use r2r::{
    example_interfaces::srv::AddTwoInts, std_msgs::msg::Int64, Context, Error, Executor, Node,
    QosProfile,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

type ApplicationError = Box<dyn std::error::Error>;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), ApplicationError> {
    let context = Context::create()?;
    let mut server = Node::create(context.clone(), "tokio_executor_server", "")?;
    let mut application = Node::create(context.clone(), "tokio_executor_gateway", "")?;
    let mut service = server.create_service::<AddTwoInts::Service>(
        "tokio_executor_sum",
        QosProfile::services_default(),
    )?;
    let client = application.create_client::<AddTwoInts::Service>(
        "tokio_executor_sum",
        QosProfile::services_default(),
    )?;
    let publisher =
        server.create_publisher::<Int64>("tokio_executor_values", QosProfile::default())?;
    let mut received =
        application.subscribe::<Int64>("tokio_executor_values", QosProfile::default())?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let mut executor = Executor::new(context.clone());
    let server_id = executor.add_node(server)?;
    let application_id = executor.add_node(application)?;
    let task = executor.spawn()?;
    let serving = async {
        for _ in 0..32 {
            let request = service.next().await.ok_or(Error::RCL_RET_SERVICE_INVALID)?;
            let sum = request.message.a + request.message.b;
            request.respond(AddTwoInts::Response { sum })?;
        }
        Ok::<_, ApplicationError>(())
    };
    let gateway = async {
        futures::try_join!(client.wait_for_service()?, publisher.wait_for_subscribers()?)?;
        let (mut socket, _) = listener.accept().await?;
        for _ in 0..32 {
            let value = socket.read_i64().await?;
            let response = client
                .request(&AddTwoInts::Request { a: value, b: 5 })?
                .await?;
            publisher.publish(&Int64 { data: response.sum })?;
            let observed = received
                .next()
                .await
                .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
            assert_eq!(observed.data, value + 5);
            socket.write_i64(observed.data).await?;
        }
        match publisher.wait_for_all_acked_async()?.await {
            Ok(()) | Err(Error::RCL_RET_UNSUPPORTED) => {}
            Err(error) => return Err(error.into()),
        }
        Ok::<_, ApplicationError>(())
    };
    let tcp_client = async {
        let mut socket = TcpStream::connect(address).await?;
        for value in -16..16 {
            socket.write_i64(value).await?;
            assert_eq!(socket.read_i64().await?, value + 5);
        }
        Ok::<_, ApplicationError>(())
    };
    let outcome = tokio::time::timeout(Duration::from_secs(5), async {
        futures::try_join!(serving, gateway, tcp_client)
    })
    .await;
    // Stop on both application success and failure, then recover ownership.
    let mut executor = tokio::time::timeout(Duration::from_secs(5), task.stop()).await??;
    outcome??;
    assert!(context.is_valid());
    executor.reset_cancel();
    // Reuse the same nodes without a runtime change or a native spin loop.
    let task = executor.spawn()?;
    let reused = async {
        let response = client.request(&AddTwoInts::Request { a: 20, b: 22 })?;
        let request = service.next().await.ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        let sum = request.message.a + request.message.b;
        request.respond(AddTwoInts::Response { sum })?;
        assert_eq!(response.await?.sum, 42);
        Ok::<_, Error>(())
    };
    let outcome = tokio::time::timeout(Duration::from_secs(5), reused).await;
    let mut executor = tokio::time::timeout(Duration::from_secs(5), task.stop()).await??;
    outcome??;
    executor.node_mut(server_id)?.destroy_publisher(publisher);
    executor
        .node_mut(application_id)?
        .destroy_subscription(received);
    drop(executor.remove_node(server_id)?);
    drop(executor.remove_node(application_id)?);
    println!("validated 32 TCP/ROS service/pubsub roundtrips and reuse on current-thread Tokio");
    Ok(())
}
