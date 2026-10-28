# ROS string bytes, fixed arrays and native storage

ROS `string` fields use `RosString`, an owned `Vec<u8>` payload. ROS `wstring` fields use
`RosWString`, an owned `Vec<u16>` payload. Native conversion preserves arbitrary bytes, embedded
NUL and unpaired UTF-16 surrogates without forcing text decoding. Use `try_to_str`, `into_string`
or `try_to_string` when the application requires text; invalid encoding returns an error.
`String` and `Vec<u8>` can move into `RosString` without another allocation.

Fixed-size ROS arrays generate `[T; N]`. Unbounded and upper-bounded sequences continue to use
`Vec<T>`. Fixed-array JSON deserialization rejects short/long arrays and cleans up partially
initialized elements on errors or unwinding; no temporary vector is needed for the array itself.

Generated message copies retain initialized native string-sequence capacity and each element's
string buffer. Updating with fewer strings, including an empty sequence, changes the logical
size without destroying hidden initialized elements. A later update within the retained capacity
can reuse them. Growth reallocates the outer array and initializes only new slots, preserving
the existing element buffers.

This benefits repeated copies into `NativeMsg<T>` and `MessageSerializer<T>`, including parameter
string arrays and message fields such as joint names. It applies to UTF-8 and UTF-16 native
sequences. UTF-16 assignment encodes directly into retained native storage, and decoding borrows
the native code units before constructing the returned Rust string. Text convenience assignment avoids a temporary UTF-16 vector, and raw code-unit assignment
copies directly into retained storage. Sequence decoding creates only the requested owned
byte/code-unit containers.
`Publisher::publish_native` can publish that reused native message directly. The
[runnable pipeline](../r2r/examples/string_sequence_pipeline.rs) sends Unicode arrays of
128, 16, 0, 32 and 128 strings across two ROS nodes and checks every complete received message.
Parameter string values also retain raw bytes. `ParameterValue::String` stores `RosString`, and
`StringArray` stores `Vec<RosString>`. Typed text reads and derived `String`/`Vec<String>` parameter
fields decode explicitly and reject invalid UTF-8 before mutating application state. Parameter
names remain text: incoming names are checked where textual semantics are required, and
permissive read services return unset/default information for invalid names.

Human-readable serde retains JSON strings for valid UTF-8/UTF-16; invalid text is represented
as an array of bytes/code units. Both forms deserialize losslessly, and out-of-range values
are rejected. Non-human-readable serde uses byte/code-unit sequences.

These transport checks validate retained storage after shrink/growth; codec timings do not
establish a transport throughput gain.

```rust
use r2r::{rcl_interfaces::msg::ParameterValue, MessageSerializer};

let mut serializer = MessageSerializer::<ParameterValue>::new()?;
let message = ParameterValue {
    type_: 9,
    string_array_value: vec!["joint_0_日本語_🦀".into()],
    ..Default::default()
};
let bytes = serializer.serialize(&message)?;
// Consume these borrowed CDR bytes before the next serialization.
```

## Ownership and failure boundaries

Retained capacity is a high-water mark for the native container's lifetime. Drop and recreate
the container/serializer to release it. Native finalizers visit all initialized capacity slots,
so shrinking the logical size does not leak retained string buffers.

If outer reallocation fails, existing values and addresses remain intact. If initialization of
a new slot fails after reallocation, newly initialized slots are finalized and the old logical
size/capacity and values remain valid. The outer array address may already have moved. Existing
element string addresses remain unchanged. A subsequent update can retry growth.

An individual string assignment can fail after earlier elements have been updated. The operation
is not a transaction across all fields; it returns the allocation error and keeps every capacity
slot valid for retry or finalization. UTF-16 buffer growth failure preserves that string's
previous value. Borrowed output and native storage cannot be reused concurrently through safe
Rust references.

## Native text and middleware codecs

Native string storage preserves embedded NUL bytes/code units according to its explicit size.
Native assignment, hidden-slot reuse and message-copy tests cover this boundary independently
from serialization.

An additional `ParameterValue` CDR counterexample remains unresolved: `"head\0tail"` survives
native copying but a Jazzy roundtrip through Rust's C type support returns `"head"` under both
CycloneDDS and Fast DDS. The committed baseline and the storage-reuse candidate behave the same.
The rclcpp roundtrip preserves the full string under CycloneDDS and truncates it under Fast DDS.
This is a codec/interoperability gap; storage reuse does not close it. Unicode string-array codec
acceptance uses NUL-free text, and the independent embedded-NUL counterexample is retained with
the benchmark evidence.

## Focused verification

```sh
cargo test -p r2r_rcl --release strings:: -- --test-threads=1
env -u CYCLONEDDS_URI cargo test -p r2r --lib --release parameter_string_arrays -- --test-threads=1
```

The [nearby native tests](../r2r_rcl/src/strings/tests.rs) compare UTF-8/UTF-16 assignment against
native `assignn`, cover surrogate pairs and invalid UTF-16, verify hidden buffers across shrink/
empty/growth, and inject allocation, new-slot initialization and UTF-16 resize failures.
The [generated-message tests](../r2r/src/msg_types/native_sequences.rs) verify native capacity,
complete values and normal Unicode CDR roundtrips for `ParameterValue`.
