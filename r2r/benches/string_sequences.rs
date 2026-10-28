#[path = "support/codec.rs"]
mod codec;
use r2r::{rcl_interfaces::msg::ParameterValue, NativeMsg};

struct Strings;
impl codec::Fixture for Strings {
    type Message = ParameterValue;
    const NAME: &'static str = "string_array";
    fn message(elements: usize) -> Self::Message {
        ParameterValue {
            type_: 9,
            string_array_value: (0..elements)
                .map(|index| format!("joint_{index:04}_日本語_🦀_name").into())
                .collect(),
            ..Default::default()
        }
    }
    fn native_pointers(value: &NativeMsg<Self::Message>) -> (usize, usize) {
        (value.string_array_value.data as usize, 0)
    }
    fn native_tag(value: &NativeMsg<Self::Message>) -> u64 {
        value.string_array_value.size as u64
    }
    fn owned_tag(value: &Self::Message) -> u64 {
        value.string_array_value.len() as u64
            ^ value.string_array_value.last().map_or(0, |text| text.len()) as u64
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    codec::run::<Strings>()
}
