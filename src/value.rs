use std::{fmt, write};

use crate::{Slice, Str};

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ValueKind {
    ValueNone,
    ValueBool,
    ValueInt,
    ValueUint,
    ValueFloat,
    ValueString,
    ValueList,
    ValueRecord,
    ValueBytes,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union Data {
    pub raw_: u64,
    pub bool_: bool,
    pub int_: i64,
    pub uint_: u64,
    pub float_: f64,
    pub string_: Str,
    pub list_: Slice<Value>,
    pub record_: Slice<Value>,
    pub bytes_: Slice<u8>,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Value {
    pub kind: ValueKind,
    pub data: Data,
}

impl Default for Value {
    fn default() -> Self {
        Self {
            kind: ValueKind::ValueNone,
            data: Data { raw_: 0 },
        }
    }
}

unsafe impl Sync for Slice<u8> {}
unsafe impl Send for Slice<u8> {}

unsafe impl Sync for Slice<Value> {}
unsafe impl Send for Slice<Value> {}

pub trait FromValue: Sized {
    fn from_value(value: &Value) -> Option<Self>;
}

impl Value {
    #[inline]
    pub fn get<T: FromValue>(&self) -> Option<T> {
        T::from_value(self)
    }
}

impl Value {
    #[inline]
    pub const fn none() -> Self {
        Self {
            kind: ValueKind::ValueNone,
            data: Data { uint_: 0 },
        }
    }

    #[inline]
    pub const fn list(values: Slice<Value>) -> Self {
        Self {
            kind: ValueKind::ValueList,
            data: Data { list_: values },
        }
    }

    /// A record and a list carry the same slice, so `From` cannot tell them
    /// apart: a record has to be built by name.
    #[inline]
    pub const fn record(fields: Slice<Value>) -> Self {
        Self {
            kind: ValueKind::ValueRecord,
            data: Data { record_: fields },
        }
    }
}

impl FromValue for bool {
    fn from_value(value: &Value) -> Option<Self> {
        if value.kind != ValueKind::ValueBool {
            return None;
        }

        Some(unsafe { value.data.bool_ })
    }
}

impl FromValue for i64 {
    fn from_value(value: &Value) -> Option<Self> {
        if value.kind != ValueKind::ValueInt {
            return None;
        }

        Some(unsafe { value.data.int_ })
    }
}

impl FromValue for u64 {
    fn from_value(value: &Value) -> Option<Self> {
        if value.kind != ValueKind::ValueUint {
            return None;
        }

        Some(unsafe { value.data.uint_ })
    }
}

impl FromValue for f64 {
    fn from_value(value: &Value) -> Option<Self> {
        if value.kind != ValueKind::ValueFloat {
            return None;
        }

        Some(unsafe { value.data.float_ })
    }
}

impl FromValue for Str {
    fn from_value(value: &Value) -> Option<Self> {
        if value.kind != ValueKind::ValueString {
            return None;
        }

        Some(unsafe { value.data.string_ })
    }
}

impl FromValue for Slice<Value> {
    fn from_value(value: &Value) -> Option<Self> {
        match value.kind {
            ValueKind::ValueList => Some(unsafe { value.data.list_ }),
            ValueKind::ValueRecord => Some(unsafe { value.data.record_ }),
            _ => None,
        }
    }
}

impl FromValue for Slice<u8> {
    fn from_value(value: &Value) -> Option<Self> {
        if value.kind != ValueKind::ValueBytes {
            return None;
        }

        Some(unsafe { value.data.bytes_ })
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self {
            kind: ValueKind::ValueBool,
            data: Data { bool_: value },
        }
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self {
            kind: ValueKind::ValueInt,
            data: Data { int_: value },
        }
    }
}

impl From<u64> for Value {
    fn from(value: u64) -> Self {
        Self {
            kind: ValueKind::ValueUint,
            data: Data { uint_: value },
        }
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self {
            kind: ValueKind::ValueFloat,
            data: Data { float_: value },
        }
    }
}

impl From<Str> for Value {
    fn from(value: Str) -> Self {
        Self {
            kind: ValueKind::ValueString,
            data: Data { string_: value },
        }
    }
}

impl From<Slice<Value>> for Value {
    fn from(value: Slice<Value>) -> Self {
        Self {
            kind: ValueKind::ValueList,
            data: Data { list_: value },
        }
    }
}

impl From<Slice<u8>> for Value {
    fn from(value: Slice<u8>) -> Self {
        Self {
            kind: ValueKind::ValueBytes,
            data: Data { bytes_: value },
        }
    }
}

const MAX_BYTES_PREVIEW: usize = 16;

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe {
            match self.kind {
                ValueKind::ValueNone => {
                    write!(f, "none")
                }
                ValueKind::ValueBool => {
                    write!(f, "{}", self.data.bool_)
                }

                ValueKind::ValueInt => {
                    write!(f, "{}", self.data.int_)
                }

                ValueKind::ValueUint => {
                    write!(f, "{}", self.data.uint_)
                }

                ValueKind::ValueFloat => {
                    write!(f, "{}", self.data.float_)
                }

                ValueKind::ValueString => {
                    write!(f, "\"{}\"", self.data.string_.as_str())
                }

                ValueKind::ValueBytes => {
                    let bytes = self.data.bytes_.as_slice();

                    write!(f, "[len={}, data=[", bytes.len())?;
                    for (i, byte) in bytes.iter().take(MAX_BYTES_PREVIEW).enumerate() {
                        if i != 0 {
                            write!(f, " ")?;
                        }

                        write!(f, "{:02X}", byte)?;
                    }

                    if bytes.len() > MAX_BYTES_PREVIEW {
                        write!(f, " ...")?;
                    }

                    write!(f, "]]")
                }

                ValueKind::ValueList => {
                    let list = self.data.list_.as_slice();

                    write!(f, "[")?;

                    for (i, value) in list.iter().enumerate() {
                        if i != 0 {
                            write!(f, ", ")?;
                        }

                        write!(f, "{value}")?;
                    }

                    write!(f, "]")
                }

                ValueKind::ValueRecord => {
                    let record = self.data.record_.as_slice();

                    write!(f, "{{")?;

                    for (i, value) in record.iter().enumerate() {
                        if i != 0 {
                            write!(f, ", ")?;
                        }

                        write!(f, "{value}")?;
                    }

                    write!(f, "}}")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Slice, Str, Value, ValueKind};

    #[test]
    fn carries_the_number_it_was_given_back() {
        assert_eq!(Value::from(7u64).get::<u64>(), Some(7));
        assert_eq!(Value::from(-7i64).get::<i64>(), Some(-7));
        assert_eq!(Value::from(0.5f64).get::<f64>(), Some(0.5));
        assert_eq!(Value::from(true).get::<bool>(), Some(true));
    }

    #[test]
    fn a_value_of_one_kind_is_not_read_as_another() {
        assert_eq!(Value::from(7u64).get::<bool>(), None);
        assert_eq!(Value::none().get::<u64>(), None);
    }

    #[test]
    fn text_survives_the_round_trip() {
        let value = Value::from(Str::from_static("h264"));
        let text = value.get::<Str>().expect("a string");

        assert_eq!(unsafe { text.as_str() }, "h264");
    }

    /// A record is not a list: the engine builds its fields by name, and one
    /// taken for the other arrives as the wrong shape.
    #[test]
    fn a_record_keeps_its_own_kind() {
        let fields = [Value::from(1u64), Value::from(2u64)];
        let slice = Slice::from_raw_parts(fields.as_ptr(), fields.len());

        assert_eq!(Value::record(slice).kind, ValueKind::ValueRecord);
        assert_eq!(Value::list(slice).kind, ValueKind::ValueList);

        // The bare conversion is a list, which is why the two exist.
        assert_eq!(Value::from(slice).kind, ValueKind::ValueList);
    }

    #[test]
    fn an_empty_slice_reads_as_empty_rather_than_crashing() {
        let empty: Slice<Value> = Slice::empty();

        assert!(unsafe { empty.as_slice() }.is_empty());
        assert!(empty.get(0).is_none());
    }

    #[test]
    fn an_empty_string_reads_as_empty() {
        assert_eq!(unsafe { Str::empty().as_str() }, "");
    }

    /// One kind per sign, so a narrower number is widened on the way in and
    /// the plugin says which it meant by casting.
    #[test]
    fn a_number_goes_in_as_the_kind_its_sign_asks_for() {
        assert_eq!(Value::from(7u64).kind, ValueKind::ValueUint);
        assert_eq!(Value::from(-7i64).kind, ValueKind::ValueInt);
        assert_eq!(Value::from(0.5f64).kind, ValueKind::ValueFloat);
        assert_eq!(Value::from(true).kind, ValueKind::ValueBool);

        assert_eq!(
            Value::from(u32::MAX as u64).get::<u64>(),
            Some(u32::MAX as u64)
        );
        assert_eq!(
            Value::from(i32::MIN as i64).get::<i64>(),
            Some(i32::MIN as i64)
        );
    }

    /// A signed value read as unsigned would come back as something enormous,
    /// so the kinds are kept apart.
    #[test]
    fn a_number_of_one_sign_is_not_read_as_the_other() {
        assert_eq!(Value::from(-1i64).get::<u64>(), None);
        assert_eq!(Value::from(1u64).get::<i64>(), None);
        assert_eq!(Value::from(1u64).get::<f64>(), None);
    }

    #[test]
    fn bytes_go_across_as_bytes_and_not_as_a_list() {
        let payload = [1u8, 2, 3];
        let value: Value = Slice::from_raw_parts(payload.as_ptr(), payload.len()).into();

        assert_eq!(value.kind, ValueKind::ValueBytes);
        assert_eq!(
            unsafe { value.get::<Slice<u8>>().unwrap().as_slice() },
            &[1, 2, 3]
        );
    }

    /// A message the engine builds starts as fields of nothing, which the
    /// plugin then fills in.
    #[test]
    fn a_value_starts_as_nothing() {
        assert_eq!(Value::default().kind, ValueKind::ValueNone);
        assert_eq!(Value::default().kind, Value::none().kind);
    }

    #[test]
    fn nothing_is_read_out_of_none() {
        let none = Value::none();

        assert_eq!(none.kind, ValueKind::ValueNone);
        assert_eq!(none.get::<u64>(), None);
        assert_eq!(none.get::<bool>(), None);
        assert!(none.get::<Str>().is_none());
        assert!(none.get::<Slice<u8>>().is_none());
    }

    #[test]
    fn a_value_says_what_it_holds_when_logged() {
        assert!(format!("{}", Value::from(7u64)).contains('7'));
        assert!(format!("{}", Value::from(Str::from_static("h264"))).contains("h264"));

        let payload = [0u8; 32];
        let bytes: Value = Slice::from_raw_parts(payload.as_ptr(), payload.len()).into();

        // A payload is said by its size: nobody wants 32 bytes in a log line.
        assert!(format!("{bytes}").contains("32"));
    }

    /// The header gives these numbers to plugins in other languages, so their
    /// order is part of the ABI.
    #[test]
    fn the_value_numbers_are_the_ones_the_header_hands_out() {
        assert_eq!(ValueKind::ValueNone as u32, 0);
        assert_eq!(ValueKind::ValueBool as u32, 1);
        assert_eq!(ValueKind::ValueInt as u32, 2);
        assert_eq!(ValueKind::ValueUint as u32, 3);
        assert_eq!(ValueKind::ValueFloat as u32, 4);
        assert_eq!(ValueKind::ValueString as u32, 5);
        assert_eq!(ValueKind::ValueList as u32, 6);
        assert_eq!(ValueKind::ValueRecord as u32, 7);
        assert_eq!(ValueKind::ValueBytes as u32, 8);
    }
}
