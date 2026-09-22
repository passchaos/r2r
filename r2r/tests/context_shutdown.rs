use r2r::{Context, Error, Node};

#[test]
fn shutdown_is_shared_idempotent_and_defers_finalization() {
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
