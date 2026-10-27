use futures::{channel::oneshot, StreamExt};
use r2r::{example_interfaces::srv::AddTwoInts, Error, FutureExecutionOptions};
use std::{cell::Cell, rc::Rc, time::Duration};

fn main() -> r2r::Result<()> {
    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create(context, "future_execution_example", "")?;
    let mut requests =
        node.create_service::<AddTwoInts::Service>("future_sum", r2r::QosProfile::default())?;
    let client =
        node.create_client::<AddTwoInts::Service>("future_sum", r2r::QosProfile::default())?;
    let options = FutureExecutionOptions::wake_driven();
    node.spin_until_future_complete_with_options(
        client.wait_for_service()?,
        Duration::from_secs(3),
        options,
    )?;

    // The external source progresses independently; the application itself needs no runtime.
    let (sender, receiver) = oneshot::channel();
    let producer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(25));
        // A deadline/error may close the receiver before this source finishes.
        let _ = sender.send(7);
    });
    let local = Rc::new(Cell::new(0));
    let server = async {
        let request = requests
            .next()
            .await
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        let sum = request.message.a + request.message.b;
        request.respond(AddTwoInts::Response { sum })?;
        Ok::<_, Error>(())
    };
    let application = async {
        let input = receiver.await.map_err(|_| Error::RCL_RET_ERROR)?;
        local.set(input);
        let response = client
            .request(&AddTwoInts::Request { a: input, b: 5 })?
            .await?;
        assert_eq!(response.sum, 12);
        Ok::<_, Error>(response.sum)
    };
    let result = node.spin_until_future_complete_with_options(
        async {
            let (_, sum) = futures::try_join!(server, application)?;
            Ok(sum)
        },
        Duration::from_secs(3),
        options,
    );
    producer.join().map_err(|_| Error::RCL_RET_ERROR)?;
    println!("external input={}, ROS result={}", local.get(), result?);
    Ok(())
}
