# Content-filtered subscription validation

`ContentFilterOptions` configures native middleware filtering for ordinary typed/native/untyped/raw
streams, scoped native callbacks and exclusive/reentrant grouped subscriptions. All public tokens
share apply/query/replace/clear methods and reject use with a foreign Node. Invalid NUL-containing
strings and more than 100 parameters fail before native mutation.

Six focused cases cover three non-breaking creation-option builders, parameter validation,
transient-local history filtered during native initialization, exact `Int32` delivery across
`data > 10` and `data < 10` live replacements, filter clearing without endpoint recreation,
native callback delivery, grouped-registry lookup and typed/native batch creation filtering. The finite example repeats the
create/replace/clear protocol. Unsupported or silently ignored RMW options must become
`RCL_RET_UNSUPPORTED`; a local decode-and-filter fallback is forbidden.

| Distribution | RMW | Native content filter | Focused tests | Example |
| --- | --- | --- | ---: | --- |
| Jazzy | Cyclone DDS | Explicitly unsupported | 6 debug + 6 release pass | Reports unsupported |
| Jazzy | Fast DDS | Supported | 6 debug + 6 release pass | Full protocol passes |
| Humble | Fast DDS | Supported | 6 pass | Full protocol passes |
| Kilted | Cyclone DDS | Explicitly unsupported | 6 pass | Reports unsupported |
| Kilted | Fast DDS | Supported | 6 pass | Full protocol passes |

Focused Clippy passes in every native environment. The workspace all-target warnings-denied gate
and the true no-ROS all-target/documentation gate also pass; the latter verifies 84 public pages
and 341 methods, including stream/callback/group/batch subscription tokens and creation configuration.

```sh
bash docs/validation/content-filtered-subscriptions-2026-10-31/run-jazzy.bash
bash docs/validation/content-filtered-subscriptions-2026-10-31/run-compat.bash
bash docs/validation/content-filtered-subscriptions-2026-10-31/run-no-ros.bash
python3 docs/validation/content-filtered-subscriptions-2026-10-31/verify.py
```

All compilation is pinned to one allowed CPU with one Cargo/build-script job and a 10 GiB
virtual/container memory ceiling. This validates API, ownership and middleware behavior; it makes
no latency or bandwidth claim against rclcpp.
