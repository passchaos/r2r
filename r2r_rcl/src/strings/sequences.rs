use super::*;

type Reallocate = unsafe extern "C" fn(
    *mut std::os::raw::c_void,
    usize,
    *mut std::os::raw::c_void,
) -> *mut std::os::raw::c_void;

macro_rules! string_sequence {
    ($ctype:ident, $to_owned:expr) => {
        paste::item! {
            impl [<$ctype __Sequence>] {
                pub fn update(&mut self, values: &[String]) {
                    let _ = self.try_update(values);
                }

                /// Copies strings while retaining all initialized sequence and string capacity.
                /// Growth failure preserves previous values. Individual assignment failure can
                /// leave a partially updated prefix; storage remains valid for retry/finalization.
                pub fn try_update(&mut self, values: &[String]) -> bool {
                    self.try_update_with(values,|target,source|target.try_assign(source))
                }
                fn try_update_with<T>(&mut self, values:&[T], mut assign:impl FnMut(&mut $ctype,&T)->bool)->bool {
                    if values.len() > self.capacity {
                        let allocator = unsafe { rcutils_get_default_allocator() };
                        if !self.try_grow_with(
                            values.len(), [<$ctype __init>],
                            allocator.reallocate.unwrap(), allocator.state,
                        ) {
                            return false;
                        }
                    }
                    if !values.is_empty() {
                        let targets = unsafe {
                            std::slice::from_raw_parts_mut(self.data, values.len())
                        };
                        for (target, source) in targets.iter_mut().zip(values) {
                            if !assign(target,source) {
                                return false;
                            }
                        }
                    }
                    // Finalizers visit capacity, so hidden slots remain initialized across
                    // empty/shrinking updates and retain their independent string buffers.
                    self.size = values.len();
                    true
                }

                pub(super) fn try_grow_with(
                    &mut self, length: usize,
                    initialize: unsafe extern "C" fn(*mut $ctype) -> bool,
                    reallocate: Reallocate, state: *mut std::os::raw::c_void,
                ) -> bool {
                    let Some(bytes) = length.checked_mul(std::mem::size_of::<$ctype>()) else {
                        return false;
                    };
                    let data = unsafe { reallocate(self.data.cast(), bytes, state) }.cast::<$ctype>();
                    if data.is_null() {
                        return false;
                    }
                    // RCL's allocator retains existing elements; initialize only new slots.
                    // Reallocation may move the outer array even if a later initialization fails.
                    self.data = data;
                    for index in self.capacity..length {
                        if !unsafe { initialize(data.add(index)) } {
                            for initialized in self.capacity..index {
                                unsafe { [<$ctype __fini>](data.add(initialized)) };
                            }
                            return false;
                        }
                    }
                    self.capacity = length;
                    true
                }

                pub fn to_vec(&self) -> Vec<String> {
                    if self.data.is_null() {
                        return Vec::new();
                    }
                    let strings = unsafe { std::slice::from_raw_parts(self.data, self.size) };
                    strings.iter().map($to_owned).collect()
                }
            }
        }
    };
}

string_sequence!(rosidl_runtime_c__String, |value: &rosidl_runtime_c__String| value
    .to_str()
    .to_owned());
string_sequence!(rosidl_runtime_c__U16String, |value: &rosidl_runtime_c__U16String| value
    .to_str());

impl rosidl_runtime_c__String__Sequence {
    pub fn try_update_bytes<T: AsRef<[u8]>>(&mut self, values: &[T]) -> bool {
        self.try_update_with(values, |target, source| target.try_assign_bytes(source.as_ref()))
    }
    pub fn to_bytes_vec<T: From<Vec<u8>>>(&self) -> Vec<T> {
        if self.data.is_null() {
            return Vec::new();
        }
        unsafe { std::slice::from_raw_parts(self.data, self.size) }
            .iter()
            .map(|value| value.as_bytes().to_vec().into())
            .collect()
    }
}
impl rosidl_runtime_c__U16String__Sequence {
    pub fn try_update_units<T: AsRef<[u16]>>(&mut self, values: &[T]) -> bool {
        self.try_update_with(values, |target, source| target.try_assign_units(source.as_ref()))
    }
    pub fn to_units_vec<T: From<Vec<u16>>>(&self) -> Vec<T> {
        if self.data.is_null() {
            return Vec::new();
        }
        unsafe { std::slice::from_raw_parts(self.data, self.size) }
            .iter()
            .map(|value| value.as_units().to_vec().into())
            .collect()
    }
}
