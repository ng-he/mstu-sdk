//! What the header promises, checked against what the crate declares.
//!
//! `abi/mstu-sdk.h` is generated from these types and is what a plugin in
//! another language is compiled against. Nothing here is used at run time:
//! it exists so that a field reordered, a width changed or a variant inserted
//! fails here rather than in somebody's plugin.

#[cfg(test)]
mod tests {
    use std::mem::{align_of, offset_of, size_of};
    use std::path::PathBuf;
    use std::process::Command;

    use crate::{HostContext, Message, Slice, Str, TypeKind, Value, ValueKind, message::Writer};

    fn header() -> String {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("abi/mstu-sdk.h");

        std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }

    /// The variants of a C enum, in the order the header gives them.
    fn variants(header: &str, name: &str) -> Vec<String> {
        let start = header
            .find(&format!("typedef enum {name} {{"))
            .unwrap_or_else(|| panic!("the header declares no {name}"));

        let body = &header[start..];
        let end = body.find('}').expect("an enum that ends");

        body[..end]
            .lines()
            .skip(1)
            .map(|line| line.trim().trim_end_matches(',').to_string())
            .filter(|line| !line.is_empty())
            .collect()
    }

    /// A variant inserted in the middle renumbers every one after it, and a
    /// plugin built against the old header goes on using the old numbers.
    #[test]
    fn the_value_kinds_are_numbered_as_the_header_numbers_them() {
        let declared = variants(&header(), "ValueKind");

        let ours = [
            (ValueKind::ValueNone, "ValueNone"),
            (ValueKind::ValueBool, "ValueBool"),
            (ValueKind::ValueInt, "ValueInt"),
            (ValueKind::ValueUint, "ValueUint"),
            (ValueKind::ValueFloat, "ValueFloat"),
            (ValueKind::ValueString, "ValueString"),
            (ValueKind::ValueList, "ValueList"),
            (ValueKind::ValueRecord, "ValueRecord"),
            (ValueKind::ValueBytes, "ValueBytes"),
        ];

        assert_eq!(declared.len(), ours.len(), "{declared:?}");

        for (number, (kind, name)) in ours.iter().enumerate() {
            assert_eq!(declared[number], *name, "variant {number}");
            assert_eq!(*kind as usize, number, "{name} is number {number}");
        }
    }

    #[test]
    fn the_type_kinds_are_numbered_as_the_header_numbers_them() {
        let declared = variants(&header(), "TypeKind");

        let ours = [
            (TypeKind::TypeBool, "TypeBool"),
            (TypeKind::TypeInt, "TypeInt"),
            (TypeKind::TypeFloat, "TypeFloat"),
            (TypeKind::TypeString, "TypeString"),
            (TypeKind::TypeRecord, "TypeRecord"),
            (TypeKind::TypeEnum, "TypeEnum"),
            (TypeKind::TypeList, "TypeList"),
            (TypeKind::TypeBytes, "TypeBytes"),
        ];

        assert_eq!(declared.len(), ours.len(), "{declared:?}");

        for (number, (kind, name)) in ours.iter().enumerate() {
            assert_eq!(declared[number], *name, "variant {number}");
            assert_eq!(*kind as usize, number, "{name} is number {number}");
        }
    }

    /// Everything that crosses the boundary is a pointer and a length, or a
    /// table of pointers. A field moved here is read as the wrong field there.
    #[test]
    fn what_crosses_the_boundary_is_laid_out_as_the_header_says() {
        let word = size_of::<usize>();

        assert_eq!(size_of::<Str>(), 2 * word);
        assert_eq!(offset_of!(Str, ptr), 0);
        assert_eq!(offset_of!(Str, len), word);

        assert_eq!(size_of::<Slice<u8>>(), 2 * word);
        assert_eq!(offset_of!(Slice<u8>, ptr), 0);
        assert_eq!(offset_of!(Slice<u8>, len), word);

        // A message is one slice of values and nothing else.
        assert_eq!(size_of::<Message>(), size_of::<Slice<Value>>());

        // The writer is the engine's own pointer and the two calls, in order.
        assert_eq!(size_of::<Writer>(), 3 * word);
        assert_eq!(offset_of!(Writer, _engine_data), 0);
        assert_eq!(offset_of!(Writer, fill), word);
        assert_eq!(offset_of!(Writer, reserve), 2 * word);

        // Five calls the host offers, each one pointer wide.
        assert_eq!(size_of::<HostContext>(), 5 * word);
        assert_eq!(align_of::<HostContext>(), word);
    }

    #[test]
    fn a_value_is_a_kind_beside_its_contents() {
        // The union holds a pointer and a length at its widest.
        assert!(size_of::<Value>() >= 2 * size_of::<usize>());
        assert_eq!(align_of::<Value>(), size_of::<usize>());
        assert_eq!(offset_of!(Value, kind), 0);
    }

    /// The header is what a plugin in another language is handed, so it has
    /// to be something a C compiler will take.
    #[test]
    fn the_header_compiles_as_c() {
        let Some(compiler) = ["cc", "gcc", "clang"]
            .into_iter()
            .find(|tool| Command::new(tool).arg("--version").output().is_ok())
        else {
            return println!("skipped: no C compiler installed");
        };

        let dir = std::env::temp_dir().join(format!("mstu-abi-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("somewhere to compile");

        let source = dir.join("check.c");

        // A plugin's whole writing path, in the shapes the header offers.
        std::fs::write(
            &source,
            r#"
#include "mstu-sdk.h"

bool write_one(struct Writer *output, const struct HostContext *host, struct Str id) {
  uint8_t *payload = mstu_reserve(output, 4);

  if (payload == NULL) {
    return false;
  }

  struct Value values[3] = {
    mstu_uint(1),
    mstu_bool(true),
    mstu_bytes(payload, 4),
  };

  mstu_publish_live(host, id, values, 3);

  return mstu_fill(output, values, 3);
}
"#,
        )
        .expect("a file to compile");

        let built = Command::new(compiler)
            .args(["-Wall", "-Wextra", "-Werror", "-c"])
            .arg("-I")
            .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("abi"))
            .arg(&source)
            .arg("-o")
            .arg(dir.join("check.o"))
            .output()
            .expect("the compiler to run");

        let _ = std::fs::remove_dir_all(&dir);

        assert!(
            built.status.success(),
            "the header does not compile as C:\n{}",
            String::from_utf8_lossy(&built.stderr)
        );
    }
}
