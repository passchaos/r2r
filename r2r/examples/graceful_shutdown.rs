use r2r::{Context, Executor, Node, NodeOptions};
use std::io::Write as _;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::create()?;
    context.install_signal_handler()?;
    let node = Node::create_with_options(
        context.clone(),
        "graceful_shutdown",
        "",
        NodeOptions::default()
            .enable_rosout(false)
            .start_type_description_service(false),
    )?;
    let mut executor = Executor::new(context.clone());
    executor.add_node(node)?;
    println!("ready; send SIGINT or SIGTERM");
    std::io::stdout().flush()?;

    let task = executor.spawn()?;
    let shutdown = context.wait_for_shutdown();
    futures::executor::block_on(shutdown);
    let executor = futures::executor::block_on(task)?;
    assert!(!context.is_valid());
    println!("shutdown complete; recovered {} node(s)", executor.len());
    Ok(())
}
