use r2r::{
    parking_lot::{Mutex, MutexGuard},
    Context, ContextOptions, Error, Node,
};

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
