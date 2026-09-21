use criterion::{black_box, criterion_group, criterion_main, Criterion};
use r2r::*;

const NUM_ELEMENTS: usize = 10_000;

fn serialized_input() -> Vec<u8> {
    let numbers = (0..NUM_ELEMENTS).map(|value| value as i32).collect();
    let message = std_msgs::msg::Int32MultiArray {
        layout: std_msgs::msg::MultiArrayLayout::default(),
        data: numbers,
    };

    message.to_serialized_bytes().unwrap()
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let bytes = serialized_input();
    c.bench_function("ros_deserialization", |b| {
        b.iter(|| {
            black_box(
                std_msgs::msg::Int32MultiArray::from_serialized_bytes(black_box(&bytes)).unwrap(),
            )
        })
    });
    c.bench_function("cdr_deserialization", |b| {
        b.iter(|| {
            black_box(
                cdr::deserialize::<std_msgs::msg::Int32MultiArray>(black_box(&bytes)).unwrap(),
            )
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
