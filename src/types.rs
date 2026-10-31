use std::{
    ffi::{CStr, c_char},
    fmt,
};

/// A borrowed UTF-8 string.
///
/// `Str` does not own the underlying memory. The caller must ensure that
/// `ptr` points to a valid UTF-8 buffer of length `len`, and that the buffer
/// remains alive for the entire time `Str` is used.
///
/// This type is intended for FFI and is ABI-compatible with a pointer-length
/// string representation.
#[repr(C)]
#[derive(Copy, Clone)]

pub struct Str {
    pub ptr: *const u8,
    pub len: usize,
}

impl Str {
    pub fn new(s: &str) -> Self {
        Self {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

    pub const fn empty() -> Self {
        Self {
            ptr: std::ptr::null(),
            len: 0,
        }
    }

    /// An empty `Str` is a null pointer, which `from_raw_parts` rejects.
    pub unsafe fn as_str<'a>(&self) -> &'a str {
        if self.ptr.is_null() || self.len == 0 {
            return "";
        }

        unsafe {
            let bytes = std::slice::from_raw_parts(self.ptr, self.len);
            std::str::from_utf8_unchecked(bytes)
        }
    }

    pub const fn from_static(s: &'static str) -> Self {
        Self {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }
}

impl fmt::Display for Str {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe { f.write_str(self.as_str()) }
    }
}

impl fmt::Debug for Str {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe { fmt::Debug::fmt(self.as_str(), f) }
    }
}

unsafe impl Sync for Str {}
unsafe impl Send for Str {}

// ABI for Str
#[unsafe(no_mangle)]
pub unsafe extern "C" fn str_from_cstr(ptr: *const c_char) -> Str {
    if ptr.is_null() {
        return Str {
            ptr: std::ptr::null(),
            len: 0,
        };
    }

    let cstr = unsafe { CStr::from_ptr(ptr) };

    Str {
        ptr: cstr.as_ptr() as *const u8,
        len: cstr.to_bytes().len(),
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Slice<T> {
    pub ptr: *const T,
    pub len: usize,
}

impl<T> Slice<T> {
    pub const fn empty() -> Self {
        Self {
            ptr: core::ptr::null(),
            len: 0,
        }
    }

    pub const fn from_raw_parts(ptr: *const T, len: usize) -> Self {
        Self { ptr, len }
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// An empty `Slice` is a null pointer, which `from_raw_parts` rejects.
    pub unsafe fn as_slice<'a>(&self) -> &'a [T] {
        if self.ptr.is_null() || self.len == 0 {
            return &[];
        }

        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }

        unsafe { Some(&*self.ptr.add(index)) }
    }

    #[inline]
    pub fn at(&self, index: usize) -> &T {
        assert!(index < self.len);
        unsafe { &*self.ptr.add(index) }
    }
}

#[macro_export]
macro_rules! slice {
    ($slice:expr) => {
        $crate::Slice::from_raw_parts(($slice).as_ptr(), ($slice).len())
    };
}

impl<T> From<&[T]> for Slice<T> {
    fn from(slice: &[T]) -> Self {
        Self {
            ptr: slice.as_ptr(),
            len: slice.len(),
        }
    }
}

impl<T> From<&mut [T]> for Slice<T> {
    fn from(slice: &mut [T]) -> Self {
        Self {
            ptr: slice.as_ptr(),
            len: slice.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_crosses_the_abi_as_a_pointer_and_a_length() {
        let owned = String::from("media file source");
        let text = Str::new(owned.as_str());

        assert_eq!(text.len, owned.len());
        assert_eq!(unsafe { text.as_str() }, "media file source");
    }

    /// An empty `Str` is a null pointer, which reading as text would follow.
    #[test]
    fn an_empty_string_is_read_without_following_its_pointer() {
        assert!(Str::empty().ptr.is_null());
        assert_eq!(unsafe { Str::empty().as_str() }, "");
        assert_eq!(unsafe { Str::from_static("").as_str() }, "");
    }

    #[test]
    fn a_static_string_needs_no_owner() {
        const CODEC: Str = Str::from_static("h264");

        assert_eq!(unsafe { CODEC.as_str() }, "h264");
        assert_eq!(CODEC.to_string(), "h264");
    }

    /// Text from C, which is terminated rather than counted.
    #[test]
    fn a_c_string_is_measured_up_to_its_terminator() {
        let text = unsafe { str_from_cstr(c"dump.h264".as_ptr()) };

        assert_eq!(unsafe { text.as_str() }, "dump.h264");
        assert_eq!(text.len, 9);
    }

    #[test]
    fn no_c_string_at_all_is_an_empty_one() {
        let text = unsafe { str_from_cstr(std::ptr::null()) };

        assert!(text.ptr.is_null());
        assert_eq!(unsafe { text.as_str() }, "");
    }

    #[test]
    fn a_slice_crosses_the_abi_the_same_way() {
        let values = [1u64, 2, 3];
        let slice = Slice::from_raw_parts(values.as_ptr(), values.len());

        assert_eq!(slice.len, 3);
        assert!(!slice.is_empty());
        assert_eq!(unsafe { slice.as_slice() }, &[1, 2, 3]);
    }

    #[test]
    fn an_empty_slice_is_read_without_following_its_pointer() {
        let empty: Slice<u64> = Slice::empty();

        assert!(empty.is_empty());
        assert!(unsafe { empty.as_slice() }.is_empty());
        assert!(empty.get(0).is_none());
    }

    #[test]
    fn reading_past_the_end_gives_nothing_rather_than_whatever_is_there() {
        let values = [7u64];
        let slice = Slice::from_raw_parts(values.as_ptr(), values.len());

        assert_eq!(slice.get(0), Some(&7));
        assert_eq!(slice.get(1), None);
        assert_eq!(slice.get(usize::MAX), None);
    }

    #[test]
    fn at_takes_only_an_index_that_is_there() {
        let values = [7u64];
        let slice = Slice::from_raw_parts(values.as_ptr(), values.len());

        assert_eq!(slice.at(0), &7);
    }

    #[test]
    #[should_panic]
    fn at_says_so_loudly_rather_than_reading_past_the_end() {
        let values = [7u64];

        Slice::from_raw_parts(values.as_ptr(), values.len()).at(1);
    }

    #[test]
    fn a_rust_slice_becomes_one_of_these() {
        let values = [1u8, 2, 3];
        let slice: Slice<u8> = (&values[..]).into();

        assert_eq!(unsafe { slice.as_slice() }, &[1, 2, 3]);

        let macroed = slice!(values);

        assert_eq!(unsafe { macroed.as_slice() }, &[1, 2, 3]);
    }

    /// `value_get`, which a plugin in another language reads a message with.
    #[test]
    fn a_value_is_reached_by_index_from_c() {
        use crate::value_get;

        let values = [1u64.into(), 2u64.into()];
        let slice = Slice::from_raw_parts(values.as_ptr(), values.len());

        let second = value_get(slice, 1);

        assert!(!second.is_null());
        assert_eq!(unsafe { &*second }.get::<u64>(), Some(2));

        assert!(value_get(slice, 2).is_null(), "past the end is nothing");
        assert!(value_get(Slice::empty(), 0).is_null());
    }
}
