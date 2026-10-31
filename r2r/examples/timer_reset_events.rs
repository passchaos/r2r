use futures::{executor::block_on, StreamExt};
use r2r::{Context, Node, TimerOptions};
use std::time::Duration;

fn main() -> r2r::Result<()> {
    let context = Context::create()?;
    let mut node = Node::create(context, "timer_reset_events", "")?;
    let timer = node.create_wall_timer_with_options(
        Duration::from_secs(30),
        TimerOptions::default().autostart(false),
    )?;

    // Late observation is lossless: both earlier resets arrive as one item.
    timer.reset(&mut node)?;
    timer.reset(&mut node)?;
    let mut resets = timer.observe_resets(&mut node)?;
    let first = block_on(resets.next()).expect("timer remains alive");
    assert_eq!((first.count, first.total), (2, 2));

    timer.cancel(&mut node)?;
    timer.reset(&mut node)?;
    let next = block_on(resets.next()).expect("timer remains alive");
    assert_eq!((next.count, next.total), (1, 3));

    node.destroy_timer(timer);
    assert!(block_on(resets.next()).is_none());
    println!("observed three resets and deterministic timer teardown");
    Ok(())
}
