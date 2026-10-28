use super::*;

#[test]
fn bytes_are_preserved_and_utf8_conversion_is_explicit() {
    let original = vec![0xff, 0, b'x', 0xfe];
    let pointer = original.as_ptr();
    let bytes = RosString::from(original);
    assert_eq!(bytes.as_ptr(), pointer);
    assert!(bytes.try_to_str().is_err());
    let error = bytes.clone().into_string().unwrap_err();
    assert_eq!(error.as_bytes(), bytes.as_bytes());
    let json = serde_json::to_vec(&bytes).unwrap();
    assert_eq!(serde_json::from_slice::<RosString>(&json).unwrap(), bytes);
    assert_eq!(bytes.into_bytes(), [0xff, 0, b'x', 0xfe]);
}

#[test]
fn valid_text_keeps_json_strings_and_zero_copy_owned_conversions() {
    let text = String::from("日本語\0🦀");
    let pointer = text.as_ptr();
    let bytes = RosString::from(text);
    assert_eq!(bytes.as_ptr(), pointer);
    let json = serde_json::to_value(&bytes).unwrap();
    assert_eq!(json, serde_json::json!("日本語\0🦀"));
    assert_eq!(serde_json::from_value::<RosString>(json).unwrap(), bytes);
    let text = bytes.into_string().unwrap();
    assert_eq!(text.as_ptr(), pointer);
    assert_eq!(text, "日本語\0🦀");
}

#[test]
fn wide_units_preserve_invalid_surrogates_and_text_roundtrips() {
    for units in [
        vec![0xd800],
        vec![0xdc00, 0],
        vec![0x4e2d, 0, 0xd83d, 0xde00],
    ] {
        let wide = RosWString::from(units.clone());
        let json = serde_json::to_value(&wide).unwrap();
        assert_eq!(
            serde_json::from_value::<RosWString>(json)
                .unwrap()
                .as_units(),
            units
        );
    }
    assert!(RosWString::from([0xd800]).try_to_string().is_err());
    let wide = RosWString::from("中文😀\0🦀");
    assert_eq!(wide.try_to_string().unwrap(), "中文😀\0🦀");
    assert_eq!(serde_json::to_value(&wide).unwrap(), serde_json::json!("中文😀\0🦀"));
}

#[test]
fn out_of_range_json_bytes_and_units_are_rejected() {
    for input in ["[-1]", "[256]", "[1.5]", "[true]"] {
        assert!(serde_json::from_str::<RosString>(input).is_err());
    }
    for input in ["[-1]", "[65536]", "[1.5]", "[true]"] {
        assert!(serde_json::from_str::<RosWString>(input).is_err());
    }
}

#[test]
fn generated_string_fields_preserve_raw_bytes_through_native_json_and_cdr() {
    use crate::{
        std_msgs::msg::String as Message, NativeMsg, WrappedNativeMsgUntyped, WrappedTypesupport,
    };
    let source = Message {
        data: RosString::from([0xff, 0xfe, 0x80, b'x']),
    };
    let native = NativeMsg::<Message>::try_from(&source).unwrap();
    assert_eq!(native.data.as_bytes(), source.data.as_bytes());
    assert_eq!(Message::from_native(&native), source);
    let json = serde_json::to_value(&source).unwrap();
    assert_eq!(json, serde_json::json!({"data":[255,254,128,120]}));
    let mut untyped = WrappedNativeMsgUntyped::new_from("std_msgs/msg/String").unwrap();
    untyped.from_json(json.clone()).unwrap();
    assert_eq!(untyped.to_json().unwrap(), json);
    assert_eq!(
        Message::from_serialized_bytes(&source.to_serialized_bytes().unwrap()).unwrap(),
        source
    );
}

#[test]
fn raw_parameter_values_roundtrip_and_text_consumers_reject_invalid_utf8() {
    use crate::{ParameterValue, RosParams};
    let raw = RosString::from([0xff, 0xfe]);
    let value = ParameterValue::from(raw.clone());
    let wire = value.clone().into_parameter_value_msg();
    assert_eq!(wire.string_value, raw);
    assert_eq!(ParameterValue::try_from_parameter_value_msg(wire).unwrap(), value);
    let recovered: RosString = value.clone().try_into().unwrap();
    assert_eq!(recovered, raw);
    let text: Result<String, _> = value.clone().try_into();
    assert!(text.is_err());
    let mut original = String::from("keep");
    assert!(original.set_parameter("", &value).is_err());
    assert_eq!(original, "keep");
    let array = ParameterValue::StringArray(vec!["valid".into(), raw]);
    let mut original = vec![String::from("keep")];
    assert!(original.set_parameter("", &array).is_err());
    assert_eq!(original, ["keep"]);
    let recovered: Vec<RosString> = array.clone().try_into().unwrap();
    assert_eq!(recovered.len(), 2);
    let text: Result<Vec<String>, _> = array.try_into();
    assert!(text.is_err());
}
