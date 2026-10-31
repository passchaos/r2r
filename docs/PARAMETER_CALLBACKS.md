# Transactional parameter callbacks

r2r applies local updates and the standard ROS `set_parameters` services through
one transaction path. Applications can transform, validate and observe live
configuration without placing mutable state behind a service callback:

```rust
node.declare_parameter("gain", 1_i64)?;
node.declare_parameter("effective_gain", 2_i64)?;

let pre = node.add_pre_set_parameters_callback(|updates| {
    if let Some(gain) = updates.iter().find_map(|(name, value)| {
        (name == "gain").then_some(value).and_then(|value| match value {
            r2r::ParameterValue::Integer(value) => Some(*value),
            _ => None,
        })
    }) {
        updates.push(("effective_gain".into(), gain.saturating_mul(2).into()));
    }
});

let validate = node.add_on_set_parameters_callback(|updates| {
    if updates.iter().any(|(name, value)| {
        name == "gain" && matches!(value, r2r::ParameterValue::Integer(value) if !(0..=10).contains(value))
    }) {
        return Err(r2r::Error::ParameterConstraintViolation {
            name: "gain".into(),
            reason: "gain must be between 0 and 10".into(),
        });
    }
    Ok(())
});

let post = node.add_post_set_parameters_callback(|updates| {
    println!("committed: {updates:?}");
});

// Callback handles are scoped registrations. Keep them alive while active.
let _callbacks = (pre, validate, post);
node.set_parameter("gain", 4_i64)?;
```

## Transaction order

For `Node::set_parameters_atomically` and the matching ROS service, r2r performs:

1. pre-set callbacks in reverse registration order;
2. duplicate-name normalization, where the final value keeps the first name's
   deterministic position;
3. descriptor, declaration and derived-parameter validation;
4. on-set callbacks in reverse registration order, stopping at the first error;
5. one atomic commit to derived and runtime parameter storage;
6. simulated-time changes, local/ROS parameter events and post-set callbacks.

Post-set callbacks run after internal parameter locks are released and only for
successful commits. `Node::set_parameters` intentionally applies each input as a
separate transaction and returns one result per item. Use the atomic method when
parameters have cross-field invariants.

Parameter declaration invokes on-set and post-set callbacks for each prepared
declaration, but not pre-set callbacks. Explicit undeclaration does not invoke
the three callback phases; it still emits deletion events. Descriptors enforce
read-only, static/dynamic typing and numeric range constraints on both local and
remote updates.

## Ownership and recursion

Each registration returns an RAII handle. Dropping it unregisters only that
callback; callbacks already captured for an active transaction finish normally.
The registry lock is not held while application code runs.

A parameter callback may inspect the committed node state. Starting another
mutation on the same node from a pre/on/post callback returns
`Error::RecursiveParameterMutation` instead of deadlocking. Mutations on other
nodes remain independent.

Run the complete remotely configurable example:

```sh
cargo run -p r2r --example parameter_callbacks -- --ros-args -r __ns:=/demo
ros2 param set /demo/parameter_callbacks gain 4
ros2 param get /demo/parameter_callbacks effective_gain
```

The scoped callback handles, deterministic ordering, transformed atomic batch,
service path, rejection rollback, event emission and recursive-mutation boundary
are covered by native tests.
