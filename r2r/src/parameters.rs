use crate::{Error, Result};
use std::ffi::CStr;

use crate::msg_types::generated_msgs::rcl_interfaces;
use indexmap::IndexMap;
use r2r_rcl::*;
pub use rcl_interfaces::msg::{FloatingPointRange, IntegerRange, ParameterDescriptor};

unsafe fn copy_native_array<T: Copy>(values: *const T, size: usize) -> Result<Vec<T>> {
    if size == 0 {
        return Ok(Vec::new());
    }
    if values.is_null() {
        return Err(Error::RCL_RET_ERROR);
    }
    Ok(std::slice::from_raw_parts(values, size).to_vec())
}

/// The kind of change represented by a ROS parameter event.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ParameterEventKind {
    New,
    Changed,
    Deleted,
}

/// A single parameter change flattened from `rcl_interfaces/msg/ParameterEvent`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterEventChange {
    pub stamp: crate::builtin_interfaces::msg::Time,
    pub node: String,
    pub kind: ParameterEventKind,
    pub name: String,
    pub value: ParameterValue,
}

pub(crate) fn flatten_parameter_event(
    event: rcl_interfaces::msg::ParameterEvent, node: Option<&str>, parameter: Option<&str>,
) -> Vec<ParameterEventChange> {
    if node.is_some_and(|expected| event.node != expected) {
        return Vec::new();
    }
    let stamp = event.stamp;
    let node = event.node;
    [
        (ParameterEventKind::New, event.new_parameters),
        (ParameterEventKind::Changed, event.changed_parameters),
        (ParameterEventKind::Deleted, event.deleted_parameters),
    ]
    .into_iter()
    .flat_map(|(kind, parameters)| {
        let stamp = stamp.clone();
        let node = node.clone();
        parameters.into_iter().filter_map(move |parameter_msg| {
            if parameter.is_some_and(|expected| parameter_msg.name != expected) {
                return None;
            }
            let name = parameter_msg.name;
            match ParameterValue::try_from_parameter_value_msg(parameter_msg.value) {
                Ok(value) => Some(ParameterEventChange {
                    stamp: stamp.clone(),
                    node: node.clone(),
                    kind,
                    name,
                    value,
                }),
                Err(err) => {
                    log::warn!("ignoring malformed parameter event for {name}: {err}");
                    None
                }
            }
        })
    })
    .collect()
}

/// ROS parameter value.
#[derive(Debug, PartialEq, Clone)]
pub enum ParameterValue {
    NotSet,
    Bool(bool),
    Integer(i64),
    Double(f64),
    String(String),
    BoolArray(Vec<bool>),
    ByteArray(Vec<u8>),
    IntegerArray(Vec<i64>),
    DoubleArray(Vec<f64>),
    StringArray(Vec<String>),
}

#[derive(Debug)]
pub struct WrongParameterType {
    pub expected_type_name: &'static str,
    pub actual_type_name: &'static str,
}

impl std::error::Error for WrongParameterType {}

impl std::fmt::Display for WrongParameterType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "got `{}`, expected `{}`", self.actual_type_name, self.expected_type_name)
    }
}

macro_rules! try_into_template {
    ($ty:ty, $expected_type_name:literal, $variant:pat => $result:expr) => {
        impl TryInto<$ty> for ParameterValue {
            type Error = WrongParameterType;

            fn try_into(self) -> std::prelude::v1::Result<$ty, Self::Error> {
                match self {
                    $variant => Ok($result),
                    _ => Err(WrongParameterType {
                        expected_type_name: $expected_type_name,
                        actual_type_name: self.type_name(),
                    }),
                }
            }
        }
    };
}

try_into_template!((), "not set", ParameterValue::NotSet => ());
try_into_template!(bool, "boolean", ParameterValue::Bool(value) => value);
try_into_template!(i64, "integer", ParameterValue::Integer(value) => value);
try_into_template!(f64, "double", ParameterValue::Double(value) => value);
try_into_template!(String, "string", ParameterValue::String(value) => value);
try_into_template!(Vec<bool>, "boolean array", ParameterValue::BoolArray(value) => value);
try_into_template!(Vec<u8>, "byte array", ParameterValue::ByteArray(value) => value);
try_into_template!(Vec<i64>, "integer array", ParameterValue::IntegerArray(value) => value);
try_into_template!(Vec<f64>, "double array", ParameterValue::DoubleArray(value) => value);
try_into_template!(Vec<String>, "string array", ParameterValue::StringArray(value) => value);

macro_rules! try_into_option_template {
    ($ty:ty, $expected_type_name:literal, $variant:pat => $result:expr) => {
        impl TryInto<Option<$ty>> for ParameterValue {
            type Error = WrongParameterType;

            fn try_into(self) -> std::prelude::v1::Result<Option<$ty>, Self::Error> {
                match self {
                    $variant => Ok(Some($result)),
                    ParameterValue::NotSet => Ok(None),
                    _ => Err(WrongParameterType {
                        expected_type_name: $expected_type_name,
                        actual_type_name: self.type_name(),
                    }),
                }
            }
        }
    };
}

try_into_option_template!(bool, "boolean", ParameterValue::Bool(value) => value);
try_into_option_template!(i64, "integer", ParameterValue::Integer(value) => value);
try_into_option_template!(f64, "double", ParameterValue::Double(value) => value);
try_into_option_template!(String, "string", ParameterValue::String(value) => value);
try_into_option_template!(Vec<bool>, "boolean array", ParameterValue::BoolArray(value) => value);
try_into_option_template!(Vec<u8>, "byte array", ParameterValue::ByteArray(value) => value);
try_into_option_template!(Vec<i64>, "integer array", ParameterValue::IntegerArray(value) => value);
try_into_option_template!(Vec<f64>, "double array", ParameterValue::DoubleArray(value) => value);
try_into_option_template!(Vec<String>, "string array", ParameterValue::StringArray(value) => value);

impl ParameterValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            ParameterValue::NotSet => "not set",
            ParameterValue::Bool(_) => "boolean",
            ParameterValue::Integer(_) => "integer",
            ParameterValue::Double(_) => "double",
            ParameterValue::String(_) => "string",
            ParameterValue::BoolArray(_) => "boolean array",
            ParameterValue::ByteArray(_) => "byte array",
            ParameterValue::IntegerArray(_) => "integer array",
            ParameterValue::DoubleArray(_) => "double array",
            ParameterValue::StringArray(_) => "string array",
        }
    }

    pub(crate) fn from_rcl(v: &rcl_variant_t) -> Result<Self> {
        let value = if !v.bool_value.is_null() {
            ParameterValue::Bool(unsafe { *v.bool_value })
        } else if !v.integer_value.is_null() {
            ParameterValue::Integer(unsafe { *v.integer_value })
        } else if !v.double_value.is_null() {
            ParameterValue::Double(unsafe { *v.double_value })
        } else if !v.string_value.is_null() {
            let s = unsafe { CStr::from_ptr(v.string_value) };
            let string = s.to_str().map_err(|err| Error::InvalidUtf8 {
                context: "parameter override value",
                err: err.to_string(),
            })?;
            let string = string.to_owned();
            ParameterValue::String(string)
        } else if !v.byte_array_value.is_null() {
            let array = unsafe { &*v.byte_array_value };
            ParameterValue::ByteArray(unsafe { copy_native_array(array.values, array.size) }?)
        } else if !v.bool_array_value.is_null() {
            let array = unsafe { &*v.bool_array_value };
            ParameterValue::BoolArray(unsafe { copy_native_array(array.values, array.size) }?)
        } else if !v.integer_array_value.is_null() {
            let array = unsafe { &*v.integer_array_value };
            ParameterValue::IntegerArray(unsafe { copy_native_array(array.values, array.size) }?)
        } else if !v.double_array_value.is_null() {
            let array = unsafe { &*v.double_array_value };
            ParameterValue::DoubleArray(unsafe { copy_native_array(array.values, array.size) }?)
        } else if !v.string_array_value.is_null() {
            let array = unsafe { &*v.string_array_value };
            let values = unsafe { copy_native_array(array.data, array.size) }?;
            let s = values
                .iter()
                .map(|cs| {
                    if cs.is_null() {
                        return Err(Error::RCL_RET_ERROR);
                    }
                    let s = unsafe { CStr::from_ptr(*cs) };
                    s.to_str()
                        .map(str::to_owned)
                        .map_err(|err| Error::InvalidUtf8 {
                            context: "parameter override string array",
                            err: err.to_string(),
                        })
                })
                .collect::<Result<Vec<_>>>()?;
            ParameterValue::StringArray(s)
        } else {
            ParameterValue::NotSet
        };
        Ok(value)
    }

    pub(crate) fn try_from_parameter_value_msg(
        msg: rcl_interfaces::msg::ParameterValue,
    ) -> Result<Self> {
        // todo: use constants from ParameterType message
        let value = match msg.type_ {
            0 => ParameterValue::NotSet,
            1 => ParameterValue::Bool(msg.bool_value),
            2 => ParameterValue::Integer(msg.integer_value),
            3 => ParameterValue::Double(msg.double_value),
            4 => ParameterValue::String(msg.string_value),
            5 => ParameterValue::ByteArray(msg.byte_array_value),
            6 => ParameterValue::BoolArray(msg.bool_array_value),
            7 => ParameterValue::IntegerArray(msg.integer_array_value),
            8 => ParameterValue::DoubleArray(msg.double_array_value),
            9 => ParameterValue::StringArray(msg.string_array_value),
            type_ => return Err(Error::UnknownParameterType { type_ }),
        };
        Ok(value)
    }

    pub(crate) fn into_parameter_value_msg(self) -> rcl_interfaces::msg::ParameterValue {
        let mut ret = rcl_interfaces::msg::ParameterValue::default();

        match self {
            ParameterValue::NotSet => {
                ret.type_ = 0; // uint8 PARAMETER_NOT_SET=0
            }
            ParameterValue::Bool(b) => {
                ret.type_ = 1; // uint8 PARAMETER_BOOL=1
                ret.bool_value = b;
            }
            ParameterValue::Integer(i) => {
                ret.type_ = 2; // uint8 PARAMETER_INTEGER=2
                ret.integer_value = i;
            }
            ParameterValue::Double(d) => {
                ret.type_ = 3; // uint8 PARAMETER_DOUBLE=3
                ret.double_value = d;
            }
            ParameterValue::String(s) => {
                ret.type_ = 4; // uint8 PARAMETER_STRING=4
                ret.string_value = s;
            }
            ParameterValue::ByteArray(ba) => {
                ret.type_ = 5; // uint8 PARAMETER_BYTE_ARRAY=5
                ret.byte_array_value = ba;
            }
            ParameterValue::BoolArray(ba) => {
                ret.type_ = 6; // uint8 PARAMETER_BOOL_ARRAY=6
                ret.bool_array_value = ba;
            }
            ParameterValue::IntegerArray(ia) => {
                ret.type_ = 7; // uint8 PARAMETER_INTEGER_ARRAY=7
                ret.integer_array_value = ia;
            }
            ParameterValue::DoubleArray(da) => {
                ret.type_ = 8; // uint8 PARAMETER_DOUBLE_ARRAY=8
                ret.double_array_value = da;
            }
            ParameterValue::StringArray(sa) => {
                ret.type_ = 9; // int PARAMETER_STRING_ARRAY=9
                ret.string_array_value = sa;
            }
        }
        ret
    }

    pub(crate) fn parameter_type(&self) -> u8 {
        match self {
            ParameterValue::NotSet => 0,          // uint8 PARAMETER_NOT_SET=0
            ParameterValue::Bool(_) => 1,         // uint8 PARAMETER_BOOL=1
            ParameterValue::Integer(_) => 2,      // uint8 PARAMETER_INTEGER=2
            ParameterValue::Double(_) => 3,       // uint8 PARAMETER_DOUBLE=3
            ParameterValue::String(_) => 4,       // uint8 PARAMETER_STRING=4
            ParameterValue::ByteArray(_) => 5,    // uint8 PARAMETER_BYTE_ARRAY=5
            ParameterValue::BoolArray(_) => 6,    // uint8 PARAMETER_BOOL_ARRAY=6
            ParameterValue::IntegerArray(_) => 7, // uint8 PARAMETER_INTEGER_ARRAY=7
            ParameterValue::DoubleArray(_) => 8,  // uint8 PARAMETER_DOUBLE_ARRAY=8
            ParameterValue::StringArray(_) => 9,  // int PARAMETER_STRING_ARRAY=9
        }
    }
}

const PARAMETER_FLOAT_RELATIVE_TOLERANCE: f64 = 1.0e-6;

fn floats_close(left: f64, right: f64) -> bool {
    left == right
        || (left.is_finite()
            && right.is_finite()
            && (left - right).abs()
                <= PARAMETER_FLOAT_RELATIVE_TOLERANCE * left.abs().max(right.abs()))
}

pub(crate) fn validate_parameter_descriptor(
    name: &str, descriptor: &ParameterDescriptor,
) -> Result<()> {
    if name.is_empty() {
        return Err(Error::InvalidParameterDescriptor {
            name: name.to_owned(),
            reason: "parameter name cannot be empty".into(),
        });
    }
    if descriptor.integer_range.len() > 1 {
        return Err(Error::InvalidParameterDescriptor {
            name: name.to_owned(),
            reason: "at most one integer range is allowed".into(),
        });
    }
    if descriptor.floating_point_range.len() > 1 {
        return Err(Error::InvalidParameterDescriptor {
            name: name.to_owned(),
            reason: "at most one floating-point range is allowed".into(),
        });
    }
    if !descriptor.integer_range.is_empty() && !descriptor.floating_point_range.is_empty() {
        return Err(Error::InvalidParameterDescriptor {
            name: name.to_owned(),
            reason: "integer and floating-point ranges are mutually exclusive".into(),
        });
    }
    if descriptor.type_ > rcl_interfaces::msg::ParameterType::PARAMETER_STRING_ARRAY as u8 {
        return Err(Error::InvalidParameterDescriptor {
            name: name.to_owned(),
            reason: format!("unknown parameter type {}", descriptor.type_),
        });
    }
    if !descriptor.dynamic_typing
        && !descriptor.integer_range.is_empty()
        && descriptor.type_ != rcl_interfaces::msg::ParameterType::PARAMETER_INTEGER as u8
    {
        return Err(Error::InvalidParameterDescriptor {
            name: name.to_owned(),
            reason: "an integer range requires an integer parameter".into(),
        });
    }
    if !descriptor.dynamic_typing
        && !descriptor.floating_point_range.is_empty()
        && descriptor.type_ != rcl_interfaces::msg::ParameterType::PARAMETER_DOUBLE as u8
    {
        return Err(Error::InvalidParameterDescriptor {
            name: name.to_owned(),
            reason: "a floating-point range requires a double parameter".into(),
        });
    }
    if let Some(range) = descriptor.floating_point_range.first() {
        if !range.from_value.is_finite() || !range.to_value.is_finite() || !range.step.is_finite() {
            return Err(Error::InvalidParameterDescriptor {
                name: name.to_owned(),
                reason: "floating-point range bounds and step must be finite".into(),
            });
        }
    }
    Ok(())
}

pub(crate) fn validate_parameter_update(
    name: &str, value: &ParameterValue, descriptor: &ParameterDescriptor, check_read_only: bool,
) -> Result<()> {
    if check_read_only && descriptor.read_only {
        return Err(Error::ParameterConstraintViolation {
            name: name.to_owned(),
            reason: "parameter is read-only".into(),
        });
    }

    let actual_type = value.parameter_type();
    if !descriptor.dynamic_typing {
        if actual_type == rcl_interfaces::msg::ParameterType::PARAMETER_NOT_SET as u8 {
            return Err(Error::ParameterConstraintViolation {
                name: name.to_owned(),
                reason: "a statically typed parameter cannot be unset".into(),
            });
        }
        if actual_type != descriptor.type_ {
            return Err(Error::ParameterConstraintViolation {
                name: name.to_owned(),
                reason: format!(
                    "wrong parameter type: expected {}, got {}",
                    descriptor.type_, actual_type
                ),
            });
        }
    }

    if let (ParameterValue::Integer(value), Some(range)) = (value, descriptor.integer_range.first())
    {
        let min = range.from_value.min(range.to_value);
        let max = range.from_value.max(range.to_value);
        if *value != min && *value != max {
            if *value < min || *value > max {
                return Err(Error::ParameterConstraintViolation {
                    name: name.to_owned(),
                    reason: format!("value {value} is outside [{min}, {max}]"),
                });
            }
            if range.step != 0
                && (i128::from(*value) - i128::from(min)) % i128::from(range.step) != 0
            {
                return Err(Error::ParameterConstraintViolation {
                    name: name.to_owned(),
                    reason: format!(
                        "value {value} does not align with step {} from {min}",
                        range.step
                    ),
                });
            }
        }
    }

    if let (ParameterValue::Double(value), Some(range)) =
        (value, descriptor.floating_point_range.first())
    {
        let min = range.from_value.min(range.to_value);
        let max = range.from_value.max(range.to_value);
        if !floats_close(*value, min) && !floats_close(*value, max) {
            if !value.is_finite() || *value < min || *value > max {
                return Err(Error::ParameterConstraintViolation {
                    name: name.to_owned(),
                    reason: format!("value {value} is outside [{min}, {max}]"),
                });
            }
            let step = range.step.abs();
            if step != 0.0 {
                let steps = ((*value - min) / step).round();
                let nearest = min + steps * step;
                if !floats_close(*value, nearest) {
                    return Err(Error::ParameterConstraintViolation {
                        name: name.to_owned(),
                        reason: format!(
                            "value {value} does not align with step {} from {min}",
                            range.step
                        ),
                    });
                }
            }
        }
    }

    Ok(())
}

/// ROS parameter.
pub struct Parameter {
    pub value: ParameterValue,
    pub description: &'static str,
    // TODO: Add other fields like min, max, step. Use field
    // attributes for defining them.
}

impl Parameter {
    pub fn new(value: ParameterValue) -> Self {
        Self {
            value,
            description: "",
        }
    }
    pub fn empty() -> Self {
        Self {
            value: ParameterValue::NotSet,
            description: "",
        }
    }
}

/// Trait for use it with
/// [`Node::make_derived_parameter_handler()`](crate::Node::make_derived_parameter_handler()).
///
/// The trait is usually derived with `r2r_macros::RosParams`. See
/// `parameters_derive.rs` example.
pub trait RosParams {
    fn register_parameters(
        &mut self, prefix: &str, param: Option<Parameter>, params: &mut IndexMap<String, Parameter>,
    ) -> Result<()>;
    fn get_parameter(&mut self, param_name: &str) -> Result<ParameterValue>;
    fn set_parameter(&mut self, param_name: &str, param_val: &ParameterValue) -> Result<()>;
    fn check_parameter(&self, param_name: &str, param_val: &ParameterValue) -> Result<()>;

    /// Validate and apply a group of parameter changes as one transaction.
    ///
    /// The default implementation restores the previous values if an implementation returns an
    /// error while applying a change after validation. Implementations with external side effects
    /// should override this method if restoring through `set_parameter` is insufficient.
    fn set_parameters_atomically(&mut self, parameters: &[(String, ParameterValue)]) -> Result<()> {
        for (name, value) in parameters {
            self.check_parameter(name, value)?;
        }

        let previous_values = parameters
            .iter()
            .map(|(name, _)| self.get_parameter(name).map(|value| (name, value)))
            .collect::<Result<Vec<_>>>()?;

        for (index, (name, value)) in parameters.iter().enumerate() {
            if let Err(err) = self.set_parameter(name, value) {
                for (old_name, old_value) in previous_values[..=index].iter().rev() {
                    if let Err(rollback_err) = self.set_parameter(old_name, old_value) {
                        log::error!(
                            "failed to roll back parameter {} after atomic update failure: {}",
                            old_name,
                            rollback_err
                        );
                    }
                }
                return Err(err);
            }
        }

        Ok(())
    }
}

// Implementation of RosParams for primitive types, i.e. leaf parameters
macro_rules! impl_ros_params {
    ($type:path, $param_value_type:path, $to_param_conv:path, $from_param_conv:path) => {
        impl RosParams for $type {
            fn register_parameters(
                &mut self, prefix: &str, param: Option<Parameter>,
                params: &mut IndexMap<String, Parameter>,
            ) -> Result<()> {
                if let Some(cli_param) = params.get(prefix) {
                    // Apply parameter value if set from command line or launch file
                    self.set_parameter("", &cli_param.value)
                        .map_err(|e| e.update_param_name(prefix))?;
                    // Remove the parameter (will be re-inserted below with deterministic order)
                    params.shift_remove(prefix);
                }
                // Insert the parameter with filled-in description etc.
                let mut param = param.unwrap();
                param.value = $param_value_type($to_param_conv(self)?);
                params.insert(prefix.to_owned(), param);
                Ok(())
            }

            fn get_parameter(&mut self, param_name: &str) -> Result<ParameterValue> {
                match param_name {
                    "" => Ok($param_value_type($to_param_conv(self)?)),
                    _ => Err(Error::InvalidParameterName {
                        name: param_name.to_owned(),
                    }),
                }
            }

            fn set_parameter(
                &mut self, param_name: &str, param_val: &ParameterValue,
            ) -> Result<()> {
                if param_name != "" {
                    return Err(Error::InvalidParameterName {
                        name: param_name.to_owned(),
                    });
                }
                match param_val {
                    $param_value_type(val) => {
                        *self = $from_param_conv(val)?;
                        Ok(())
                    }
                    _ => Err(Error::InvalidParameterType {
                        name: "".to_string(), // will be completed by callers who know the name
                        ty: std::stringify!($param_value_type),
                    }),
                }
            }

            fn check_parameter(&self, param_name: &str, param_val: &ParameterValue) -> Result<()> {
                if param_name != "" {
                    return Err(Error::InvalidParameterName {
                        name: param_name.to_owned(),
                    });
                }
                match param_val {
                    $param_value_type(val) => {
                        let _: Self = $from_param_conv(val)?;
                        Ok(())
                    }
                    _ => Err(Error::InvalidParameterType {
                        name: "".to_string(), // will be completed by callers who know the name
                        ty: std::stringify!($param_value_type),
                    }),
                }
            }
        }
    };
}

impl_ros_params!(bool, ParameterValue::Bool, noop, noop);
impl_ros_params!(i8, ParameterValue::Integer, try_conv, try_conv);
impl_ros_params!(i16, ParameterValue::Integer, try_conv, try_conv);
impl_ros_params!(i32, ParameterValue::Integer, try_conv, try_conv);
impl_ros_params!(i64, ParameterValue::Integer, noop, noop);
impl_ros_params!(u8, ParameterValue::Integer, try_conv, try_conv);
impl_ros_params!(u16, ParameterValue::Integer, try_conv, try_conv);
impl_ros_params!(u32, ParameterValue::Integer, try_conv, try_conv);
impl_ros_params!(f64, ParameterValue::Double, noop, noop);
impl_ros_params!(f32, ParameterValue::Double, to_f64, to_f32);
impl_ros_params!(String, ParameterValue::String, to_string, to_string);
impl_ros_params!(Vec<bool>, ParameterValue::BoolArray, clone_value, clone_value);
impl_ros_params!(Vec<u8>, ParameterValue::ByteArray, clone_value, clone_value);
impl_ros_params!(Vec<i64>, ParameterValue::IntegerArray, clone_value, clone_value);
impl_ros_params!(Vec<f64>, ParameterValue::DoubleArray, clone_value, clone_value);
impl_ros_params!(Vec<String>, ParameterValue::StringArray, clone_value, clone_value);

// Helper conversion functions
fn noop<T: Copy>(x: &T) -> Result<T> {
    Ok(*x)
}

fn to_f32(x: &f64) -> Result<f32> {
    Ok(*x as f32)
}
fn to_f64(x: &f32) -> Result<f64> {
    Ok(*x as f64)
}

fn try_conv<T, U>(x: &T) -> Result<U>
where
    T: Copy,
    U: TryFrom<T>,
    <U as TryFrom<T>>::Error: std::error::Error,
{
    U::try_from(*x).map_err(|e| Error::ParameterValueConv {
        name: "".into(),
        msg: e.to_string(),
    })
}

fn to_string(x: &str) -> Result<String> {
    Ok(x.to_string())
}

fn clone_value<T: Clone>(value: &T) -> Result<T> {
    Ok(value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingParams {
        first: i64,
        second: i64,
    }

    impl RosParams for FailingParams {
        fn register_parameters(
            &mut self, _prefix: &str, _param: Option<Parameter>,
            _params: &mut IndexMap<String, Parameter>,
        ) -> Result<()> {
            Ok(())
        }

        fn get_parameter(&mut self, name: &str) -> Result<ParameterValue> {
            match name {
                "first" => Ok(ParameterValue::Integer(self.first)),
                "second" => Ok(ParameterValue::Integer(self.second)),
                _ => Err(Error::InvalidParameterName { name: name.into() }),
            }
        }

        fn set_parameter(&mut self, name: &str, value: &ParameterValue) -> Result<()> {
            let ParameterValue::Integer(value) = value else {
                return Err(Error::InvalidParameterType {
                    name: name.into(),
                    ty: "integer",
                });
            };
            match name {
                "first" => self.first = *value,
                "second" if *value == 99 => {
                    return Err(Error::ParameterValueConv {
                        name: name.into(),
                        msg: "simulated setter failure".into(),
                    });
                }
                "second" => self.second = *value,
                _ => return Err(Error::InvalidParameterName { name: name.into() }),
            }
            Ok(())
        }

        fn check_parameter(&self, name: &str, value: &ParameterValue) -> Result<()> {
            if !matches!(value, ParameterValue::Integer(_)) {
                return Err(Error::InvalidParameterType {
                    name: name.into(),
                    ty: "integer",
                });
            }
            match name {
                "first" | "second" => Ok(()),
                _ => Err(Error::InvalidParameterName { name: name.into() }),
            }
        }
    }

    #[test]
    fn atomic_parameter_update_rolls_back_prior_changes() {
        let mut params = FailingParams {
            first: 1,
            second: 2,
        };
        let updates = vec![
            ("first".into(), ParameterValue::Integer(10)),
            ("second".into(), ParameterValue::Integer(99)),
        ];

        assert!(params.set_parameters_atomically(&updates).is_err());
        assert_eq!(params.first, 1);
        assert_eq!(params.second, 2);
    }

    #[test]
    fn array_parameters_roundtrip_and_reject_wrong_types() {
        let mut strings = vec!["first".to_owned(), "second".to_owned()];
        let mut registered = IndexMap::new();
        strings
            .register_parameters(
                "names",
                Some(Parameter::new(ParameterValue::NotSet)),
                &mut registered,
            )
            .unwrap();

        assert_eq!(
            registered.get("names").map(|parameter| &parameter.value),
            Some(&ParameterValue::StringArray(strings.clone()))
        );
        strings
            .set_parameter("", &ParameterValue::StringArray(vec!["replacement".to_owned()]))
            .unwrap();
        assert_eq!(strings, vec!["replacement"]);
        assert!(strings
            .check_parameter("", &ParameterValue::IntegerArray(vec![1]))
            .is_err());
    }

    #[test]
    fn malformed_native_parameter_arrays_are_rejected() {
        let mut bytes = rcl_byte_array_t {
            values: std::ptr::null_mut(),
            size: 1,
        };
        let variant = rcl_variant_t {
            bool_value: std::ptr::null_mut(),
            integer_value: std::ptr::null_mut(),
            double_value: std::ptr::null_mut(),
            string_value: std::ptr::null_mut(),
            byte_array_value: &mut bytes,
            bool_array_value: std::ptr::null_mut(),
            integer_array_value: std::ptr::null_mut(),
            double_array_value: std::ptr::null_mut(),
            string_array_value: std::ptr::null_mut(),
        };
        assert!(matches!(ParameterValue::from_rcl(&variant), Err(Error::RCL_RET_ERROR)));

        bytes.size = 0;
        assert_eq!(ParameterValue::from_rcl(&variant).unwrap(), ParameterValue::ByteArray(vec![]));

        let mut null_string = std::ptr::null_mut();
        let mut strings = rcutils_string_array_t {
            size: 1,
            data: &mut null_string,
            allocator: unsafe { rcutils_get_default_allocator() },
        };
        let variant = rcl_variant_t {
            bool_value: std::ptr::null_mut(),
            integer_value: std::ptr::null_mut(),
            double_value: std::ptr::null_mut(),
            string_value: std::ptr::null_mut(),
            byte_array_value: std::ptr::null_mut(),
            bool_array_value: std::ptr::null_mut(),
            integer_array_value: std::ptr::null_mut(),
            double_array_value: std::ptr::null_mut(),
            string_array_value: &mut strings,
        };
        assert!(matches!(ParameterValue::from_rcl(&variant), Err(Error::RCL_RET_ERROR)));
    }

    fn descriptor_for(value: &ParameterValue) -> ParameterDescriptor {
        ParameterDescriptor {
            name: "value".into(),
            type_: value.parameter_type(),
            ..Default::default()
        }
    }

    #[test]
    fn integer_descriptor_accepts_endpoints_and_exact_steps() {
        let mut descriptor = descriptor_for(&ParameterValue::Integer(0));
        descriptor.integer_range = vec![IntegerRange {
            from_value: 10,
            to_value: -10,
            step: 4,
        }];

        for value in [-10, -6, -2, 2, 6, 10] {
            validate_parameter_update("value", &ParameterValue::Integer(value), &descriptor, true)
                .unwrap();
        }
        for value in [-11, -5, 9, 11] {
            assert!(validate_parameter_update(
                "value",
                &ParameterValue::Integer(value),
                &descriptor,
                true,
            )
            .is_err());
        }

        descriptor.integer_range[0] = IntegerRange {
            from_value: i64::MIN,
            to_value: i64::MAX,
            step: u64::MAX,
        };
        validate_parameter_update("value", &ParameterValue::Integer(i64::MAX), &descriptor, true)
            .unwrap();
    }

    #[test]
    fn floating_descriptor_uses_ros_compatible_step_tolerance() {
        let mut descriptor = descriptor_for(&ParameterValue::Double(0.0));
        descriptor.floating_point_range = vec![FloatingPointRange {
            from_value: 0.0,
            to_value: 1.0,
            step: 0.1,
        }];

        validate_parameter_update(
            "value",
            &ParameterValue::Double(0.30000000000000004),
            &descriptor,
            true,
        )
        .unwrap();
        assert!(validate_parameter_update(
            "value",
            &ParameterValue::Double(0.35),
            &descriptor,
            true,
        )
        .is_err());
        assert!(validate_parameter_update(
            "value",
            &ParameterValue::Double(f64::NAN),
            &descriptor,
            true,
        )
        .is_err());
        assert!(validate_parameter_update(
            "value",
            &ParameterValue::Double(f64::INFINITY),
            &descriptor,
            true,
        )
        .is_err());
    }

    #[test]
    fn descriptor_enforces_read_only_and_static_typing() {
        let mut descriptor = descriptor_for(&ParameterValue::Integer(1));
        descriptor.read_only = true;
        assert!(matches!(
            validate_parameter_update("value", &ParameterValue::Integer(2), &descriptor, true),
            Err(Error::ParameterConstraintViolation { .. })
        ));
        validate_parameter_update("value", &ParameterValue::Integer(1), &descriptor, false)
            .unwrap();

        descriptor.read_only = false;
        assert!(validate_parameter_update(
            "value",
            &ParameterValue::Double(1.0),
            &descriptor,
            true,
        )
        .is_err());
        assert!(validate_parameter_update("value", &ParameterValue::NotSet, &descriptor, true,)
            .is_err());

        descriptor.dynamic_typing = true;
        validate_parameter_update("value", &ParameterValue::Double(1.0), &descriptor, true)
            .unwrap();
    }

    #[test]
    fn malformed_descriptors_are_rejected() {
        let mut descriptor = descriptor_for(&ParameterValue::Integer(1));
        descriptor.integer_range = vec![IntegerRange::default(), IntegerRange::default()];
        assert!(validate_parameter_descriptor("value", &descriptor).is_err());

        descriptor.integer_range.truncate(1);
        descriptor.floating_point_range = vec![FloatingPointRange::default()];
        assert!(validate_parameter_descriptor("value", &descriptor).is_err());

        descriptor.integer_range.clear();
        descriptor.floating_point_range[0].step = f64::INFINITY;
        assert!(validate_parameter_descriptor("value", &descriptor).is_err());
    }

    #[test]
    fn unknown_wire_parameter_types_are_rejected() {
        let malformed = rcl_interfaces::msg::ParameterValue {
            type_: u8::MAX,
            integer_value: 42,
            ..Default::default()
        };
        assert!(matches!(
            ParameterValue::try_from_parameter_value_msg(malformed.clone()),
            Err(Error::UnknownParameterType { type_: u8::MAX })
        ));

        let event = rcl_interfaces::msg::ParameterEvent {
            new_parameters: vec![rcl_interfaces::msg::Parameter {
                name: "malformed".into(),
                value: malformed,
            }],
            ..Default::default()
        };
        assert!(flatten_parameter_event(event, None, None).is_empty());
    }
}
