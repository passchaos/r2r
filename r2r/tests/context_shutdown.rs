use futures::FutureExt;
use r2r::{
    parking_lot::{Mutex, MutexGuard},
    Context, ContextOptions, Error, Node,
};
use std::time::{Duration, Instant};

fn context_test_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock()
}

#[test]
fn shutdown_is_shared_idempotent_and_defers_finalization() {
    let _lock = context_test_lock();
    let context = Context::create().unwrap();
    let clone = context.clone();
    let node = Node::create(context.clone(), "context_shutdown_test", "").unwrap();
    assert!(context.is_valid());
    assert!(clone.is_valid());

    context.shutdown().unwrap();
    assert!(!context.is_valid());
    assert!(!clone.is_valid());
    #[cfg(unix)]
    assert!(matches!(context.install_signal_handler(), Err(Error::RCL_RET_NOT_INIT)));
    clone.shutdown().unwrap();

    assert!(matches!(Context::create(), Err(Error::RCL_RET_ALREADY_SHUTDOWN)));

    drop(context);
    drop(clone);
    drop(node);

    let fresh = Context::create().unwrap();
    assert!(fresh.is_valid());
}

#[test]
fn context_options_apply_arguments_and_domain_id() {
    let _lock = context_test_lock();
    let options = ContextOptions::default()
        .arguments(["r2r-test", "--ros-args", "-r", "__ns:=/configured"])
        .domain_id(37);
    let context = Context::create_with_options(options.clone()).unwrap();
    let node = Node::create(context.clone(), "configured_context", "").unwrap();

    assert_eq!(node.namespace().unwrap(), "/configured");
    assert_eq!(node.domain_id().unwrap(), 37);
    assert!(Context::create_with_options(options).is_ok());
    assert!(matches!(Context::create(), Err(Error::RCL_RET_ALREADY_INIT)));
    assert!(matches!(
        Context::create_with_options(ContextOptions::default().domain_id(38)),
        Err(Error::RCL_RET_ALREADY_INIT)
    ));
}

#[test]
fn context_options_reject_arguments_with_nul_bytes() {
    let _lock = context_test_lock();
    assert!(matches!(
        Context::create_with_options(ContextOptions::default().arguments(["r2r\0test"])),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
}

#[test]
fn reports_spin_results_after_context_shutdown() {
    let _lock = context_test_lock();
    let context = Context::create().unwrap();
    let mut empty = Node::create(context.clone(), "shutdown_empty_spin", "").unwrap();
    let mut waitable = Node::create(context.clone(), "shutdown_waitable_spin", "").unwrap();
    let _timer = waitable.create_wall_timer(Duration::from_secs(1)).unwrap();

    context.shutdown().unwrap();

    let started = Instant::now();
    let empty_result = empty.try_spin_once(Duration::from_secs(1));
    let waitable_result = waitable.try_spin_once(Duration::ZERO);

    assert!(matches!(empty_result, Err(Error::RCL_RET_NOT_INIT)));
    assert!(matches!(waitable_result, Err(Error::RCL_RET_NOT_INIT)));
    assert!(started.elapsed() < Duration::from_millis(100));
}

#[test]
fn shutdown_wakes_an_empty_node_spin() {
    let _lock = context_test_lock();
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "shutdown_wakes_empty_spin", "").unwrap();
    let shutdown_context = context.clone();
    let shutdown = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        shutdown_context.shutdown().unwrap();
    });

    let started = Instant::now();
    let result = node.try_spin_once(Duration::from_secs(5));
    let elapsed = started.elapsed();
    shutdown.join().unwrap();

    assert!(matches!(result, Err(Error::RCL_RET_NOT_INIT)));
    assert!(elapsed >= Duration::from_millis(10));
    assert!(elapsed < Duration::from_millis(500));
}

#[test]
fn independently_created_context_handle_wakes_an_empty_node_spin() {
    let _lock = context_test_lock();
    let context = Context::create().unwrap();
    let shutdown_context = Context::create().unwrap();
    let mut node = Node::create(context, "shared_shutdown_wakes_empty_spin", "").unwrap();
    let shutdown = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        shutdown_context.shutdown().unwrap();
    });

    let started = Instant::now();
    let result = node.try_spin_once(Duration::from_secs(5));
    let elapsed = started.elapsed();
    shutdown.join().unwrap();

    assert!(matches!(result, Err(Error::RCL_RET_NOT_INIT)));
    assert!(elapsed >= Duration::from_millis(10));
    assert!(elapsed < Duration::from_millis(500));
}

#[test]
fn shutdown_completes_background_task_and_rejects_new_spawn() {
    let _lock = context_test_lock();
    let context = Context::create().unwrap();
    let node = Node::create(context.clone(), "shutdown_background_task", "").unwrap();
    let mut executor = r2r::Executor::new(context.clone());
    executor.add_node(node).unwrap();
    let task = executor.spawn().unwrap();
    let shutdown_context = context.clone();
    let shutdown = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        shutdown_context.shutdown().unwrap();
    });
    let (finished, completed) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        finished.send(futures::executor::block_on(task)).unwrap();
    });
    let executor = completed
        .recv_timeout(Duration::from_secs(5))
        .expect("shutdown did not wake background executor")
        .unwrap();
    assert_eq!(executor.len(), 1);
    assert!(!context.is_valid());
    assert!(matches!(
        executor.spawn(),
        Err(r2r::ExecutorTaskError::Native(Error::RCL_RET_NOT_INIT))
    ));
    shutdown.join().unwrap();
    worker.join().unwrap();
}

#[cfg(unix)]
#[test]
fn process_signal_shuts_down_context_and_wakes_background_executor() {
    let _lock = context_test_lock();
    let context = Context::create().unwrap();
    context.install_signal_handler().unwrap();
    let node = Node::create(context.clone(), "signal_shutdown_background_task", "").unwrap();
    let mut executor = r2r::Executor::new(context.clone());
    executor.add_node(node).unwrap();
    let task = executor.spawn().unwrap();
    let (finished, completed) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        finished.send(futures::executor::block_on(task)).unwrap();
    });

    signal_hook::low_level::raise(signal_hook::consts::SIGINT).unwrap();

    let executor = completed
        .recv_timeout(Duration::from_secs(5))
        .expect("SIGINT did not wake the background executor")
        .unwrap();
    assert_eq!(executor.len(), 1);
    assert!(!context.is_valid());
    worker.join().unwrap();
    drop(executor);
    drop(context);

    let fresh = Context::create().unwrap();
    fresh.install_signal_handler().unwrap();
    let shutdown = fresh.wait_for_shutdown();
    signal_hook::low_level::raise(signal_hook::consts::SIGTERM).unwrap();
    futures::executor::block_on(shutdown);
    assert!(!fresh.is_valid(), "SIGTERM did not retarget the process handler");
}

#[test]
fn shutdown_future_is_immediate_cancel_safe_and_does_not_retain_context() {
    let _lock = context_test_lock();
    let context = Context::create().unwrap();
    let canceled = context.wait_for_shutdown();
    drop(canceled);
    let wait = context.wait_for_shutdown();
    context.shutdown().unwrap();
    futures::executor::block_on(wait);
    assert!(context.wait_for_shutdown().now_or_never().is_some());
    drop(context);

    let context = Context::create().unwrap();
    let wait = context.wait_for_shutdown();
    drop(context);
    futures::executor::block_on(wait);
    assert!(Context::create().unwrap().is_valid());
}
