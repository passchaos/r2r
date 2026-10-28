#[cfg(r2r_bench_serializer)]
use r2r::MessageSerializer;
use r2r::{NativeMsg, WrappedTypesupport};
#[path = "allocation_probe.rs"]
mod allocation_probe;
use allocation_probe::{NativeStats, Probe};
use std::{hint::black_box, time::Instant};

pub trait Fixture {
    type Message: WrappedTypesupport + PartialEq + std::fmt::Debug;
    const NAME: &'static str;
    fn message(elements: usize) -> Self::Message;
    fn native_pointers(value: &NativeMsg<Self::Message>) -> (usize, usize);
    fn native_tag(value: &NativeMsg<Self::Message>) -> u64;
    fn owned_tag(value: &Self::Message) -> u64;
}

fn fingerprint(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf29ce484222325, |value, byte| {
        (value ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

pub fn run<F: Fixture>() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.len() < 3 || arguments.len() > 4 {
        return Err("usage: ros_comparison <copy_reuse|serialize_owned|serialize_reuse|serialize_owned_reuse|deserialize_owned> <elements> <iterations> [wire-file]".into());
    }
    let mode = &arguments[0];
    let elements: usize = arguments[1].parse()?;
    let iterations: usize = arguments[2].parse()?;
    if iterations == 0 || elements > u32::MAX as usize {
        return Err("invalid benchmark dimensions".into());
    }
    let source = F::message(elements);
    let rmw = unsafe { std::ffi::CStr::from_ptr(r2r_rcl::rmw_get_implementation_identifier()) }
        .to_str()?;
    let bytes = source.to_serialized_bytes()?;
    assert_eq!(F::Message::from_serialized_bytes(&bytes)?, source);
    if let Some(path) = arguments.get(3) {
        std::fs::write(path, &bytes)?;
    }
    let mut native = NativeMsg::<F::Message>::try_from(&source)?;
    #[cfg(r2r_bench_serializer)]
    let mut serializer = MessageSerializer::<F::Message>::new()?;
    #[cfg(r2r_bench_serializer)]
    assert_eq!(serializer.serialize(&source)?, bytes);
    let copy_pointers = F::native_pointers(&native);
    let mut copy_reallocations = 0;
    let mut checksum = 0_u64;
    let mut operation = || -> r2r::Result<u64> {
        match mode.as_str() {
            "copy_reuse" => {
                source.try_copy_to_native(black_box(&mut native))?;
                if F::native_pointers(&native) != copy_pointers {
                    copy_reallocations += 1;
                }
                Ok(F::native_tag(&native))
            }
            "serialize_owned" => {
                let output = black_box(&source).to_serialized_bytes()?;
                Ok(output.len() as u64 ^ u64::from(*output.last().unwrap()))
            }
            "serialize_reuse" | "serialize_owned_reuse" => {
                #[cfg(r2r_bench_serializer)]
                {
                    let output = serializer.serialize(black_box(&source))?;
                    if mode == "serialize_owned_reuse" {
                        let output = black_box(output.to_vec());
                        Ok(output.len() as u64 ^ u64::from(*output.last().unwrap()))
                    } else {
                        Ok(output.len() as u64 ^ u64::from(*output.last().unwrap()))
                    }
                }
                #[cfg(not(r2r_bench_serializer))]
                {
                    // Historical public API always returns owned bytes. Consume the same
                    // result without retaining it, including its allocation/copy cost.
                    let output = black_box(&source).to_serialized_bytes()?;
                    Ok(output.len() as u64 ^ u64::from(*output.last().unwrap()))
                }
            }
            "deserialize_owned" => {
                let output = F::Message::from_serialized_bytes(black_box(&bytes))?;
                Ok(F::owned_tag(&output))
            }
            _ => Err(r2r::Error::RCL_RET_INVALID_ARGUMENT),
        }
    };
    for _ in 0..100 {
        black_box(operation()?);
    }
    let mut batches = Vec::with_capacity(iterations.div_ceil(1000));
    let probe = Probe::load()?;
    if let Some(probe) = &probe {
        if unsafe { (probe.begin)() } != 0 {
            return Err("cannot start codec allocation frame".into());
        }
    }
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
    let allocation_frame = if let Some(probe) = &probe {
        let mut stats = NativeStats::default();
        if unsafe { (probe.end)(&mut stats) } != 0 {
            return Err("cannot stop codec allocation frame".into());
        }
        Some(serde_json::json!({
            "malloc_calls":stats.malloc_calls,"calloc_calls":stats.calloc_calls,
            "realloc_calls":stats.realloc_calls,"free_calls":stats.free_calls,
            "aligned_calls":stats.aligned_calls,"failed_allocations":stats.failed_allocations,
            "requested_bytes":stats.requested_bytes,"size_overflows":stats.size_overflows,
            "counter_overflows":stats.counter_overflows,
        }))
    } else {
        None
    };
    if mode == "copy_reuse" {
        assert_eq!(F::Message::from_native(&native), source);
    }
    #[cfg(r2r_bench_serializer)]
    assert_eq!(serializer.serialize(&source)?, bytes);
    batches.sort_by(f64::total_cmp);
    let percentile = |percent: usize| batches[((batches.len() - 1) * percent).div_ceil(100)];
    println!(
        "{}",
        serde_json::json!({
            "implementation": "r2r", "fixture": F::NAME, "mode": mode, "elements": elements, "iterations": iterations,
        "rmw": rmw,
            "elapsed_ns": elapsed, "ns_per_op": elapsed as f64 / iterations as f64,
            "batch_ns_per_op_p50": percentile(50), "batch_ns_per_op_p95": percentile(95), "batch_ns_per_op_p99": percentile(99),
            "wire_bytes": bytes.len(), "wire_hash": fingerprint(&bytes), "checksum": checksum,
            "native_copy_pointer_changes": copy_reallocations,
            "allocation_frame": allocation_frame,
        })
    );
    Ok(())
}
