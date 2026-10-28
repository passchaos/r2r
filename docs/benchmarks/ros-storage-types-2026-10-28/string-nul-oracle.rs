use r2r::WrappedTypesupport;
fn main() {
 let source = r2r::rcl_interfaces::msg::ParameterValue { type_: 9,
  string_array_value: vec!["head\0tail".into()], ..Default::default() };
 let native = r2r::NativeMsg::try_from(&source).unwrap();
 let bytes = source.to_serialized_bytes().unwrap();
 let observed = r2r::rcl_interfaces::msg::ParameterValue::from_serialized_bytes(&bytes).unwrap();
 println!("{{\"native_matches\":{},\"cdr_matches\":{},\"wire_bytes\":{},\"input_bytes\":{},\"observed_bytes\":{}}}",
 r2r::rcl_interfaces::msg::ParameterValue::from_native(&native)==source, observed==source,
 bytes.len(), source.string_array_value[0].len(), observed.string_array_value[0].len());
}
