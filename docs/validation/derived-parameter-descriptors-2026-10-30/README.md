# Derived parameter descriptor validation

`#[derive(RosParams)]` now propagates leaf-field `#[ros_param(...)]` metadata into standard ROS
`ParameterDescriptor` values. Supported attributes cover read-only fields, additional constraints,
one integer range or one floating-point range with mandatory `from`, `to` and `step` entries.
Unknown, duplicate, incomplete and mutually exclusive options fail during macro expansion.

The handler registers descriptors before applying CLI/YAML overrides. Matching overrides are
validated before the user struct is changed; a rejected value leaves provider state and handler
ownership unchanged. Explicit descriptors already installed through the Node API take precedence.
Hand-written `RosParams` implementations remain source compatible through a default no-op method.

The real-RCL case uses a derived double range and read-only integer range. It checks a valid
override, invalid override rollback, local success/rejection, `describe_parameters` contents and
per-entry `set_parameters` outcomes. It runs under Jazzy/both RMWs, Humble/Fast DDS and
Kilted/both RMWs. Macro parser tests and the updated nested derive example compile in every target.

```sh
bash docs/validation/derived-parameter-descriptors-2026-10-30/run-jazzy.bash
bash docs/validation/derived-parameter-descriptors-2026-10-30/run-compat.bash
bash docs/validation/derived-parameter-descriptors-2026-10-30/run-no-ros.bash
python3 docs/validation/derived-parameter-descriptors-2026-10-30/verify.py
```

Every compilation is pinned to one CPU with Cargo/build-script parallelism set to one and a 10 GiB
virtual/container memory ceiling. `results.json` binds final sources and compressed gate logs.
