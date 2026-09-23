use criterion::{black_box, criterion_group, criterion_main, Criterion};
use r2r::MessageInfo;
use r2r_rcl::rmw_message_info_t;

fn message_info_conversion(c: &mut Criterion) {
    let mut native = rmw_message_info_t {
        source_timestamp: 11,
        received_timestamp: 22,
        publication_sequence_number: 33,
        reception_sequence_number: 44,
        ..Default::default()
    };
    for (index, byte) in native.publisher_gid.data.iter_mut().enumerate() {
        *byte = index as u8;
    }

    c.bench_function("message_info_from_rmw", |b| {
        b.iter(|| black_box(MessageInfo::from(black_box(native))))
    });
}

criterion_group!(benches, message_info_conversion);
criterion_main!(benches);
