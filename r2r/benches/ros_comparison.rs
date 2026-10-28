#[path = "support/codec.rs"]
mod codec;
use r2r::{
    std_msgs::msg::{Int32MultiArray, MultiArrayDimension, MultiArrayLayout},
    NativeMsg,
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

struct Integers;
impl codec::Fixture for Integers {
    type Message = Int32MultiArray;
    const NAME: &'static str = "int32_array";
    fn message(elements: usize) -> Self::Message {
        message(elements)
    }
    fn native_pointers(value: &NativeMsg<Self::Message>) -> (usize, usize) {
        (value.data.data as usize, value.layout.dim.data as usize)
    }
    fn native_tag(value: &NativeMsg<Self::Message>) -> u64 {
        value.data.size as u64 ^ value.layout.dim.size as u64
    }
    fn owned_tag(value: &Self::Message) -> u64 {
        value.data.len() as u64 ^ value.data.last().copied().unwrap_or(0) as u64
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    codec::run::<Integers>()
}
