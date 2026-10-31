use std::{ffi::c_void, fmt, write, writeln};

use crate::{Slice, Value};

#[repr(C)]
pub struct Message {
    pub values: Slice<Value>,
}

impl Message {
    pub fn values_slice(&self) -> &[Value] {
        unsafe { self.values.as_slice() }
    }

    pub fn values_slice_mut(&mut self) -> &mut [Value] {
        // An empty message is a null pointer, which from_raw_parts rejects.
        if self.values.ptr.is_null() || self.values.len == 0 {
            return &mut [];
        }

        unsafe { std::slice::from_raw_parts_mut(self.values.ptr as *mut Value, self.values.len) }
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let values = unsafe { self.values.as_slice() };

        writeln!(f, "Message {{")?;

        for (i, value) in values.iter().enumerate() {
            writeln!(f, "  [{i}] = {value}")?;
        }

        write!(f, "}}")
    }
}

/// One line, for logs. `Display` stays multi-line for the console.
impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let values = unsafe { self.values.as_slice() };

        write!(f, "[")?;

        for (i, value) in values.iter().enumerate() {
            if i != 0 {
                write!(f, ", ")?;
            }

            write!(f, "{value}")?;
        }

        write!(f, "]")
    }
}

/// Fills in a message. The engine owns every byte one carries: a plugin
/// either lets `fill` copy what it has, or writes into `reserve`d memory.
#[repr(C)]
pub struct Writer {
    /// Reserved for engine use.
    /// Plugins must never read or modify this field.
    pub _engine_data: *mut c_void,

    /// Writes the whole message, one value per field, in order.
    ///
    /// What the values point at is copied in, except memory from `reserve`,
    /// which the message already owns. False if the count is wrong.
    pub fill: extern "C" fn(w: *mut Writer, message: Message) -> bool,

    /// Engine memory for `len` bytes, living as long as the message.
    ///
    /// A value handed to `fill` may point at it, and nothing is copied.
    pub reserve: extern "C" fn(w: *mut Writer, len: usize) -> *mut u8,
}

impl Writer {
    /// One value per field, in order.
    #[inline]
    pub fn fill(&mut self, values: &[Value]) -> bool {
        let message = Message {
            values: Slice::from_raw_parts(values.as_ptr(), values.len()),
        };

        (self.fill)(self, message)
    }

    /// Engine memory to write a payload into, with no copy afterwards.
    #[inline]
    pub fn reserve(&mut self, len: usize) -> Option<&mut [u8]> {
        let buffer = (self.reserve)(self, len);

        if buffer.is_null() {
            return None;
        }

        Some(unsafe { core::slice::from_raw_parts_mut(buffer, len) })
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use crate::{Slice, Str, Value};

    use super::*;

    fn message(values: &[Value]) -> Message {
        Message {
            values: Slice::from_raw_parts(values.as_ptr(), values.len()),
        }
    }

    #[test]
    fn a_message_is_read_field_by_field() {
        let values = [7u64.into(), Str::from_static("a").into()];
        let message = message(&values);

        assert_eq!(message.values_slice().len(), 2);
        assert_eq!(message.values_slice()[0].get::<u64>(), Some(7));
    }

    /// An empty message is a null pointer, which slicing would follow.
    #[test]
    fn an_empty_message_is_read_without_following_its_pointer() {
        let empty = Message {
            values: Slice::empty(),
        };

        assert!(empty.values_slice().is_empty());

        let mut empty = Message {
            values: Slice::empty(),
        };

        assert!(empty.values_slice_mut().is_empty());
    }

    #[test]
    fn a_field_can_be_written_where_it_lies() {
        let mut values = [Value::none(), Value::none()];

        // Over memory the caller owns and may write, as the engine hands it.
        let mut message = Message {
            values: Slice::from_raw_parts(values.as_mut_ptr(), values.len()),
        };

        message.values_slice_mut()[1] = 9u64.into();

        assert_eq!(message.values_slice()[1].get::<u64>(), Some(9));

        // The message wrote through to whoever owns the values.
        assert_eq!(values[1].get::<u64>(), Some(9));
        assert_eq!(values[0].get::<u64>(), None);
    }

    #[test]
    fn a_message_says_what_it_holds_when_logged() {
        let values = [7u64.into(), Str::from_static("h264").into()];
        let message = message(&values);

        let one_line = format!("{message:?}");

        assert!(one_line.starts_with('['), "{one_line}");
        assert!(one_line.contains("h264"), "{one_line}");
        assert!(!one_line.contains('\n'), "a log line stays on its line");

        let laid_out = format!("{message}");

        assert!(laid_out.contains("[0]"), "{laid_out}");
        assert!(laid_out.contains('\n'), "the console gets it spread out");
    }

    // ---------- the writer, as a plugin in Rust uses it ----------

    thread_local! {
        /// What the engine standing in for the real one was handed.
        static FILLED: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
        static RESERVED: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    }

    extern "C" fn fill(_: *mut Writer, message: Message) -> bool {
        FILLED.with(|filled| *filled.borrow_mut() = message.values_slice().to_vec());

        true
    }

    extern "C" fn refuse(_: *mut Writer, _: Message) -> bool {
        false
    }

    extern "C" fn reserve(_: *mut Writer, len: usize) -> *mut u8 {
        RESERVED.with(|reserved| {
            let mut reserved = reserved.borrow_mut();

            *reserved = vec![0; len];
            reserved.as_mut_ptr()
        })
    }

    extern "C" fn refuse_to_reserve(_: *mut Writer, _: usize) -> *mut u8 {
        std::ptr::null_mut()
    }

    fn writer(
        fill: extern "C" fn(*mut Writer, Message) -> bool,
        reserve: extern "C" fn(*mut Writer, usize) -> *mut u8,
    ) -> Writer {
        Writer {
            _engine_data: std::ptr::null_mut(),
            fill,
            reserve,
        }
    }

    #[test]
    fn filling_hands_every_value_to_the_engine() {
        let mut writer = writer(fill, reserve);

        assert!(writer.fill(&[1u64.into(), Str::from_static("a").into()]));

        FILLED.with(|filled| {
            let filled = filled.borrow();

            assert_eq!(filled.len(), 2);
            assert_eq!(filled[0].get::<u64>(), Some(1));
        });
    }

    #[test]
    fn a_message_the_engine_will_not_take_says_so() {
        let mut writer = writer(refuse, reserve);

        assert!(!writer.fill(&[1u64.into()]));
    }

    #[test]
    fn reserving_gives_back_memory_of_the_length_asked_for() {
        let mut writer = writer(fill, reserve);

        let room = writer.reserve(16).expect("engine memory");

        assert_eq!(room.len(), 16);

        room[0] = 0xAB;

        RESERVED.with(|reserved| assert_eq!(reserved.borrow()[0], 0xAB));
    }

    /// The engine refusing is a null pointer, which must not become a slice.
    #[test]
    fn memory_the_engine_will_not_give_is_none_rather_than_a_null_slice() {
        let mut writer = writer(fill, refuse_to_reserve);

        assert!(writer.reserve(16).is_none());
    }
}
