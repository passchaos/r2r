use std::time::Duration;

use futures::{stream, StreamExt, TryStreamExt};
use r2r::{example_interfaces::srv::AddTwoInts, QosProfile};

fn main() -> r2r::Result<()> {
    const REQUESTS: i64 = 64;
    const IN_FLIGHT: usize = 16;

    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create(context, "service_pipeline_example", "")?;
    let qos = QosProfile::services_default().keep_last(IN_FLIGHT);
    let mut service = node.create_service::<AddTwoInts::Service>("pipeline_sum", qos.clone())?;
    let client = node.create_client::<AddTwoInts::Service>("pipeline_sum", qos)?;
    let available = client.wait_for_service()?;
    node.spin_until_future_complete(available, Duration::from_secs(3))?;

    let server = async move {
        for _ in 0..REQUESTS {
            let request = service
                .next()
                .await
                .ok_or(r2r::Error::RCL_RET_SERVICE_INVALID)?;
            let sum = request.message.a + request.message.b;
            request.respond(AddTwoInts::Response { sum })?;
        }
        Ok::<_, r2r::Error>(())
    };
    let requester = async move {
        // futures-rs supplies bounded concurrency without selecting an async runtime.
        // Each request owns a cancellation-aware response future; dropping the combined
        // operation on timeout removes pending response registrations.
        let mut responses: Vec<_> = stream::iter(0..REQUESTS)
            .map(|sequence| {
                let response = client.request(&AddTwoInts::Request { a: sequence, b: 5 });
                async move {
                    let response = response?.await?;
                    Ok::<_, r2r::Error>((sequence, response.sum))
                }
            })
            .buffer_unordered(IN_FLIGHT)
            .try_collect()
            .await?;
        responses.sort_unstable();
        for (sequence, sum) in responses {
            assert_eq!(sum, sequence + 5);
        }
        Ok::<_, r2r::Error>(())
    };
    node.spin_until_future_complete(
        async {
            futures::try_join!(server, requester)?;
            Ok(())
        },
        Duration::from_secs(5),
    )?;
    println!("completed {REQUESTS} requests with at most {IN_FLIGHT} in flight");
    Ok(())
}
