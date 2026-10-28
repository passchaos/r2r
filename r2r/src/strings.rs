//! Owned ROS string payloads with explicit text decoding.
use serde::{
    de::{SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{
    fmt,
    ops::{Deref, DerefMut},
};

/// An owned ROS `string` payload. Bytes need not be UTF-8 and may contain NUL.
#[repr(transparent)]
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RosString(Vec<u8>);
impl RosString {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
    pub fn try_to_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.0)
    }
    pub fn into_string(self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.0)
    }
    pub(crate) fn into_text(self, context: &'static str) -> crate::Result<String> {
        self.into_string()
            .map_err(|error| crate::Error::InvalidUtf8 {
                context,
                err: error.to_string(),
            })
    }
}
impl Deref for RosString {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for RosString {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl AsRef<[u8]> for RosString {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
impl From<Vec<u8>> for RosString {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}
impl From<String> for RosString {
    fn from(text: String) -> Self {
        Self(text.into_bytes())
    }
}
impl From<&String> for RosString {
    fn from(text: &String) -> Self {
        Self::from(text.as_str())
    }
}
impl TryFrom<RosString> for String {
    type Error = std::string::FromUtf8Error;
    fn try_from(value: RosString) -> Result<Self, Self::Error> {
        value.into_string()
    }
}
impl From<&str> for RosString {
    fn from(text: &str) -> Self {
        Self(text.as_bytes().to_vec())
    }
}
impl From<&[u8]> for RosString {
    fn from(bytes: &[u8]) -> Self {
        Self(bytes.to_vec())
    }
}
impl<const N: usize> From<[u8; N]> for RosString {
    fn from(bytes: [u8; N]) -> Self {
        Self(bytes.into())
    }
}
impl PartialEq<str> for RosString {
    fn eq(&self, other: &str) -> bool {
        self.0 == other.as_bytes()
    }
}
impl PartialEq<&str> for RosString {
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}
impl PartialEq<String> for RosString {
    fn eq(&self, other: &String) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}
impl PartialEq<RosString> for String {
    fn eq(&self, other: &RosString) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}
impl fmt::Debug for RosString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_to_str() {
            Ok(text) => f.debug_tuple("RosString").field(&text).finish(),
            Err(_) => f.debug_tuple("RosString").field(&self.0).finish(),
        }
    }
}
impl fmt::Display for RosString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_to_str() {
            Ok(text) => f.write_str(text),
            Err(_) => write!(f, "{:?}", self),
        }
    }
}
impl Serialize for RosString {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            if let Ok(text) = self.try_to_str() {
                return serializer.serialize_str(text);
            }
        }
        serializer.serialize_bytes(&self.0)
    }
}
impl<'de> Deserialize<'de> for RosString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Bytes;
        impl<'de> Visitor<'de> for Bytes {
            type Value = RosString;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("text or a sequence of raw bytes")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(value.into())
            }
            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(value.into())
            }
            fn visit_bytes<E: serde::de::Error>(self, value: &[u8]) -> Result<Self::Value, E> {
                Ok(value.into())
            }
            fn visit_byte_buf<E: serde::de::Error>(self, value: Vec<u8>) -> Result<Self::Value, E> {
                Ok(value.into())
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut values: A) -> Result<Self::Value, A::Error> {
                let mut bytes = Vec::new();
                while let Some(value) = values.next_element::<u8>()? {
                    bytes.push(value);
                }
                Ok(bytes.into())
            }
        }
        if deserializer.is_human_readable() {
            deserializer.deserialize_any(Bytes)
        } else {
            deserializer.deserialize_byte_buf(Bytes)
        }
    }
}

/// An owned ROS `wstring` payload represented as UTF-16 code units.
#[repr(transparent)]
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct RosWString(Vec<u16>);
impl RosWString {
    pub fn as_units(&self) -> &[u16] {
        &self.0
    }
    pub fn into_units(self) -> Vec<u16> {
        self.0
    }
    pub fn try_to_string(&self) -> Result<String, std::string::FromUtf16Error> {
        String::from_utf16(&self.0)
    }
}
impl Deref for RosWString {
    type Target = Vec<u16>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for RosWString {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl AsRef<[u16]> for RosWString {
    fn as_ref(&self) -> &[u16] {
        &self.0
    }
}
impl From<Vec<u16>> for RosWString {
    fn from(units: Vec<u16>) -> Self {
        Self(units)
    }
}
impl From<&[u16]> for RosWString {
    fn from(units: &[u16]) -> Self {
        Self(units.to_vec())
    }
}
impl From<&str> for RosWString {
    fn from(text: &str) -> Self {
        Self(text.encode_utf16().collect())
    }
}
impl From<String> for RosWString {
    fn from(text: String) -> Self {
        Self::from(text.as_str())
    }
}
impl PartialEq<str> for RosWString {
    fn eq(&self, other: &str) -> bool {
        self.0.iter().copied().eq(other.encode_utf16())
    }
}
impl PartialEq<&str> for RosWString {
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}
impl<const N: usize> From<[u16; N]> for RosWString {
    fn from(units: [u16; N]) -> Self {
        Self(units.into())
    }
}
struct WideText<'a>(&'a [u16]);
impl fmt::Display for WideText<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for character in char::decode_utf16(self.0.iter().copied()) {
            let character = character.map_err(|_| fmt::Error)?;
            f.write_str(character.encode_utf8(&mut [0; 4]))?;
        }
        Ok(())
    }
}
impl Serialize for RosWString {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable()
            && char::decode_utf16(self.0.iter().copied()).all(|value| value.is_ok())
        {
            return serializer.collect_str(&WideText(&self.0));
        }
        self.0.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for RosWString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Units;
        impl<'de> Visitor<'de> for Units {
            type Value = RosWString;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("text or a sequence of UTF-16 code units")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(value.into())
            }
            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(value.into())
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut values: A) -> Result<Self::Value, A::Error> {
                let mut units = Vec::new();
                while let Some(value) = values.next_element::<u16>()? {
                    units.push(value);
                }
                Ok(units.into())
            }
        }
        if deserializer.is_human_readable() {
            deserializer.deserialize_any(Units)
        } else {
            Vec::<u16>::deserialize(deserializer).map(Self)
        }
    }
}

#[cfg(test)]
mod tests;
