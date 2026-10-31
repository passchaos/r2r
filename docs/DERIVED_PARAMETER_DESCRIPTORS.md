# Descriptor constraints in `RosParams` derive

`#[derive(r2r::RosParams)]` accepts `#[ros_param(...)]` on leaf fields so the Rust configuration
declaration and standard ROS parameter descriptor cannot drift apart:

```rust
#[derive(r2r::RosParams, Debug)]
struct ControllerParameters {
    /// Calibrated controller gain.
    #[ros_param(
        floating_point_range(from = 0.0, to = 1.0, step = 0.1),
        additional_constraints = "finite calibration"
    )]
    gain: f64,

    #[ros_param(read_only, integer_range(from = 0, to = 100, step = 5))]
    actuator_count: i64,
}
```

Supported options are:

- `read_only` or `read_only = true|false`;
- `additional_constraints = "..."`;
- `integer_range(from = ..., to = ..., step = ...)`;
- `floating_point_range(from = ..., to = ..., step = ...)`.

Range entries require all three values, and integer/floating ranges are mutually exclusive.
Malformed, duplicate or unknown options are compile errors. Rust type checking validates the
literal/expression types used to construct the standard descriptor. Applying a range to an
incompatible leaf type is rejected during handler setup by the same runtime descriptor validator
used by explicit Node declarations.

Doc comments remain the ROS descriptor description. Attributes belong on leaf parameter fields;
metadata on a nested parameter group is intentionally not inherited by all descendants. An
explicit descriptor already registered through the Node API takes precedence over derived
metadata.

## Transaction and service behavior

Derived descriptors are registered before CLI/YAML overrides are applied to the user struct.
Matching overrides are checked for type, range and step first, with read-only disabled only for
this initial declaration phase. A rejected override therefore leaves the struct, Node parameter
map, descriptors and parameter service endpoints unchanged.

After setup, local mutations and the standard `set_parameters`/
`set_parameters_atomically` services use the existing shared transaction path. Read-only fields
reject updates; numeric values must satisfy endpoints/step tolerance; failed atomic requests roll
back all values. `describe_parameters` returns the generated description, constraints, range,
type and read-only flag.

Hand-written `RosParams` implementations remain source compatible because
`register_parameter_descriptors` has a no-op default. Custom implementations can override it to
participate in the same declaration path. Built-in leaf parameter implementations fill native
name/type information and keep explicit descriptors intact.

The updated [`parameters_derive`](../r2r/examples/parameters_derive.rs) example demonstrates all
three attribute families.
