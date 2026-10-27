use std::{hint::black_box, time::Instant};

use r2r::{
    std_msgs::msg::{Int32MultiArray, MultiArrayDimension, MultiArrayLayout},
    NativeMsg, WrappedTypesupport,
};

fn message(elements: usize) -> Int32MultiArray {
    Int32MultiArray {
        layout: MultiArrayLayout {
            dim: vec![MultiArrayDimension {
                label: "samples".into(),
                size: elements as u32,
                stride: elements as u32,
            }],
            data_offset: 0,
        },
        data: (0..elements)
            .map(|index| ((index * 17 + 3) % 100_003) as i32 - 50_000)
            .collect(),
    }
}

fn fingerprint(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf29ce484222325, |value, byte| {
        (value ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.len() < 3 || arguments.len() > 4 {
        return Err("usage: ros_comparison <copy_reuse|serialize_owned|deserialize_owned> <elements> <iterations> [wire-file]".into());
    }
    let mode = &arguments[0];
    let elements: usize = arguments[1].parse()?;
    let iterations: usize = arguments[2].parse()?;
    if iterations == 0 || elements > u32::MAX as usize {
        return Err("invalid benchmark dimensions".into());
    }
    let source = message(elements);
    let rmw = unsafe { std::ffi::CStr::from_ptr(r2r_rcl::rmw_get_implementation_identifier()) }
        .to_str()?;
    let bytes = source.to_serialized_bytes()?;
    assert_eq!(Int32MultiArray::from_serialized_bytes(&bytes)?, source);
    if let Some(path) = arguments.get(3) {
        std::fs::write(path, &bytes)?;
    }
    let mut native = NativeMsg::<Int32MultiArray>::try_from(&source)?;
    let copy_pointer = native.data.data;
    let copy_dimension_pointer = native.layout.dim.data;
    let mut copy_reallocations = 0;
    let mut checksum = 0_u64;
    let mut operation = || -> r2r::Result<u64> {
        match mode.as_str() {
            "copy_reuse" => {
                source.try_copy_to_native(black_box(&mut native))?;
                if native.data.data != copy_pointer
                    || native.layout.dim.data != copy_dimension_pointer
                {
                    copy_reallocations += 1;
                }
                Ok(native.data.size as u64 ^ native.layout.dim.size as u64)
            }
            "serialize_owned" => {
                let output = black_box(&source).to_serialized_bytes()?;
                Ok(output.len() as u64 ^ u64::from(*output.last().unwrap()))
            }
            "deserialize_owned" => {
                let output = Int32MultiArray::from_serialized_bytes(black_box(&bytes))?;
                Ok(output.data.len() as u64
                    ^ output.data.last().copied().unwrap_or_default() as u64)
            }
            _ => Err(r2r::Error::RCL_RET_INVALID_ARGUMENT),
        }
    };
    for _ in 0..100 {
        black_box(operation()?);
    }
    let mut batches = Vec::new();
    let started = Instant::now();
    let mut completed = 0;
    while completed < iterations {
        let count = 1000.min(iterations - completed);
        let batch = Instant::now();
        for _ in 0..count {
            checksum = checksum.wrapping_add(black_box(operation()?));
        }
        batches.push(batch.elapsed().as_nanos() as f64 / count as f64);
        completed += count;
    }
    let elapsed = started.elapsed().as_nanos();
    if mode == "copy_reuse" {
        assert_eq!(Int32MultiArray::from_native(&native), source);
    }
    batches.sort_by(f64::total_cmp);
    let percentile = |percent: usize| batches[((batches.len() - 1) * percent).div_ceil(100)];
    println!(
        "{}",
        serde_json::json!({
            "implementation": "r2r", "mode": mode, "elements": elements, "iterations": iterations,
        "rmw": rmw,
            "elapsed_ns": elapsed, "ns_per_op": elapsed as f64 / iterations as f64,
            "batch_ns_per_op_p50": percentile(50), "batch_ns_per_op_p95": percentile(95), "batch_ns_per_op_p99": percentile(99),
            "wire_bytes": bytes.len(), "wire_hash": fingerprint(&bytes), "checksum": checksum,
            "native_copy_pointer_changes": copy_reallocations,
        })
    );
    Ok(())
}
