use serde::{
    de::{Error, IgnoredAny, SeqAccess, Visitor},
    ser::SerializeTuple,
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{fmt, marker::PhantomData, mem::MaybeUninit};

pub(super) fn serialize<S: Serializer, T: Serialize, const N: usize>(
    values: &[T; N], serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut tuple = serializer.serialize_tuple(N)?;
    for value in values {
        tuple.serialize_element(value)?;
    }
    tuple.end()
}

struct PartialArray<T, const N: usize> {
    values: [MaybeUninit<T>; N],
    initialized: usize,
}
impl<T, const N: usize> Drop for PartialArray<T, N> {
    fn drop(&mut self) {
        for value in &mut self.values[..self.initialized] {
            unsafe { value.assume_init_drop() };
        }
    }
}

pub(super) fn deserialize<'de, D: Deserializer<'de>, T: Deserialize<'de>, const N: usize>(
    deserializer: D,
) -> Result<[T; N], D::Error> {
    struct ArrayVisitor<T, const N: usize>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const N: usize> Visitor<'de> for ArrayVisitor<T, N> {
        type Value = [T; N];
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "an array of exactly {N} elements")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
            let mut array: PartialArray<T, N> = PartialArray {
                values: std::array::from_fn(|_| MaybeUninit::uninit()),
                initialized: 0,
            };
            for index in 0..N {
                let value = sequence
                    .next_element()?
                    .ok_or_else(|| A::Error::invalid_length(index, &self))?;
                array.values[index].write(value);
                array.initialized += 1;
            }
            if sequence.next_element::<IgnoredAny>()?.is_some() {
                return Err(A::Error::invalid_length(N + 1, &self));
            }
            // All N elements are initialized. Transfer their ownership and disarm error cleanup.
            let result = unsafe { array.values.as_ptr().cast::<[T; N]>().read() };
            array.initialized = 0;
            Ok(result)
        }
    }
    deserializer.deserialize_tuple(N, ArrayVisitor::<T, N>(PhantomData))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Arrays {
        #[serde(with = "super")]
        values: [u16; 36],
        #[serde(with = "super")]
        empty: [u8; 0],
    }
    #[test]
    fn large_and_empty_arrays_roundtrip_without_a_sequence_length_field() {
        let value = Arrays {
            values: std::array::from_fn(|index| index as u16),
            empty: [],
        };
        let bytes = serde_json::to_vec(&value).unwrap();
        assert_eq!(serde_json::from_slice::<Arrays>(&bytes).unwrap(), value);
    }
    #[test]
    fn short_long_and_invalid_elements_fail_at_the_array_boundary() {
        for values in [vec![0; 35], vec![0; 37], vec![70_000; 36]] {
            let value = serde_json::json!({"values":values,"empty":[]});
            assert!(serde_json::from_value::<Arrays>(value).is_err());
        }
        let value = serde_json::json!({"values":vec![0;36],"empty":[1]});
        assert!(serde_json::from_value::<Arrays>(value).is_err());
    }

    #[test]
    fn partially_initialized_elements_are_dropped_on_error_and_unwind() {
        thread_local! { static DROPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
        struct Element(Box<u32>);
        impl Drop for Element {
            fn drop(&mut self) {
                assert!(*self.0 < 999);
                DROPS.with(|drops| drops.set(drops.get() + 1));
            }
        }
        impl<'de> Deserialize<'de> for Element {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = u32::deserialize(deserializer)?;
                if value == 999 {
                    return Err(D::Error::custom("injected element error"));
                }
                assert_ne!(value, 998, "injected element panic");
                Ok(Self(Box::new(value)))
            }
        }
        for input in ["[1,2]", "[1,2,999]", "[1,2,3,4]"] {
            DROPS.with(|drops| drops.set(0));
            let mut reader = serde_json::Deserializer::from_str(input);
            assert!(deserialize::<_, Element, 3>(&mut reader).is_err());
            let expected = if input == "[1,2,3,4]" { 3 } else { 2 };
            assert_eq!(DROPS.with(|drops| drops.get()), expected);
        }
        DROPS.with(|drops| drops.set(0));
        assert!(std::panic::catch_unwind(|| {
            let mut reader = serde_json::Deserializer::from_str("[1,2,998]");
            let _ = deserialize::<_, Element, 3>(&mut reader);
        })
        .is_err());
        assert_eq!(DROPS.with(|drops| drops.get()), 2);
        DROPS.with(|drops| drops.set(0));
        let mut reader = serde_json::Deserializer::from_str("[1,2,3]");
        let values = deserialize::<_, Element, 3>(&mut reader).ok().unwrap();
        assert_eq!(DROPS.with(|drops| drops.get()), 0);
        drop(values);
        assert_eq!(DROPS.with(|drops| drops.get()), 3);
    }
}
