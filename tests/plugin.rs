//! A plugin written in C, built here and loaded here.
//!
//! The header is the only thing a plugin in another language is handed, so
//! this builds `plugin.c` against it and nothing else, twice: once with a C
//! compiler and once with a C++ one, because a C++ plugin that exports a
//! mangled `plugin_descriptor` is a plugin the engine cannot find.

use std::assert_eq;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::process::Command;

use libloading::Library;
use mstu_sdk::{Message, PluginDescriptorFn, Slice, Str, TypeKind, Value, ValueKind, message};

/// What the plugin wrote, kept so the test can read it back.
///
/// Stands in for the engine, which owns every byte a message carries: the
/// plugin is handed a writer of ours and allocates nothing for the message.
#[derive(Default)]
struct Engine {
    values: Vec<Value>,
    reserved: Vec<u8>,

    /// The nested arrays of a list or a record, boxed so they keep their
    /// address as more arrive.
    kept: Vec<Box<[Value]>>,
}

/// Copies the spine of a list or a record, as the engine's arena does.
///
/// A plugin builds those arrays on its own stack and they are gone the moment
/// `process` returns, so a writer that only copies the top level reads freed
/// memory. Leaves are left alone: a string points at the plugin's static data
/// and a payload at memory `reserve` handed out, and both outlive the call.
fn keep(kept: &mut Vec<Box<[Value]>>, values: &[Value]) -> Slice<Value> {
    let mut copied = values.to_vec();

    for value in &mut copied {
        if matches!(value.kind, ValueKind::ValueList | ValueKind::ValueRecord) {
            let inner = unsafe { value.data.list_.as_slice() };

            value.data.list_ = keep(kept, inner);
        }
    }

    let boxed = copied.into_boxed_slice();
    let slice = Slice::from_raw_parts(boxed.as_ptr(), boxed.len());

    kept.push(boxed);

    slice
}

extern "C" fn fill(writer: *mut message::Writer, msg: Message) -> bool {
    let engine = unsafe { &mut *((*writer)._engine_data as *mut Engine) };

    let spine = keep(&mut engine.kept, unsafe { msg.values.as_slice() });
    engine.values = unsafe { spine.as_slice() }.to_vec();

    true
}

extern "C" fn reserve(writer: *mut message::Writer, len: usize) -> *mut u8 {
    let engine = unsafe { &mut *((*writer)._engine_data as *mut Engine) };

    engine.reserved = vec![0; len];
    engine.reserved.as_mut_ptr()
}

fn writer(engine: &mut Engine) -> message::Writer {
    message::Writer {
        _engine_data: (engine as *mut Engine).cast::<c_void>(),
        fill,
        reserve,
    }
}

/// The first of `tools` that answers, or nothing: a machine without a
/// compiler has nothing to test rather than something broken.
fn compiler(tools: &[&str]) -> Option<String> {
    tools
        .iter()
        .find(|tool| {
            Command::new(tool)
                .arg("--version")
                .output()
                .is_ok_and(|out| out.status.success())
        })
        .map(|tool| tool.to_string())
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mstu-plugin-{}-{name}", std::process::id()));

    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("somewhere to build");

    dir
}

/// Builds `plugin.c` into a shared library with `tool`, as `standard` says.
fn build(tool: &str, standard: &str, into: &Path) -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let library = into.join(format!("plugin{}", std::env::consts::DLL_SUFFIX));

    let built = Command::new(tool)
        .args([standard, "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC"])
        .arg("-I")
        .arg(crate_dir.join("abi"))
        .arg(crate_dir.join("tests/plugin.c"))
        .arg("-o")
        .arg(&library)
        .output()
        .expect("the compiler to run");

    assert!(
        built.status.success(),
        "{tool} {standard} would not build a plugin from the header:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );

    library
}

/// Everything the engine asks of a plugin, asked of this one.
fn exercise(path: &Path) {
    let library = unsafe { Library::new(path) }.expect("the plugin to load");

    // Unmangled, or a C++ plugin is invisible however well it compiled.
    let descriptor = unsafe {
        let entry = library
            .get::<PluginDescriptorFn>(b"plugin_descriptor")
            .expect("plugin_descriptor to be exported under its own name");

        &*entry()
    };

    let metadata = (descriptor.metadata)();
    assert_eq!(unsafe { metadata.name.as_str() }, "C fixture");
    assert_eq!(unsafe { metadata.version.as_str() }, "0.1.0");

    schema_reads_back(descriptor);
    settings_read_back(descriptor);
    a_message_is_written(descriptor);
}

/// The list of records, which nothing had declared before this.
fn schema_reads_back(descriptor: &mstu_sdk::PluginDescriptor) {
    let output = unsafe { &*(descriptor.process_output_schema)() };
    let fields = unsafe { output.fields.as_slice() };

    assert_eq!(fields.len(), 3);
    assert_eq!(unsafe { fields[0].name.as_str() }, "start_time");
    assert_eq!(fields[0].ty.kind, TypeKind::TypeInt);
    assert!(!unsafe { fields[0].ty.schema.int_ }.signed);
    assert_eq!(unsafe { fields[0].ty.schema.int_ }.bits, 64);

    assert_eq!(unsafe { fields[2].name.as_str() }, "objects");
    assert_eq!(fields[2].ty.kind, TypeKind::TypeList);

    let element = unsafe { fields[2].ty.schema.list_ }.element;
    assert_eq!(element.kind, TypeKind::TypeRecord);

    let record = unsafe { &*(element.schema.record_).schema };
    let inner = unsafe { record.fields.as_slice() };

    assert_eq!(inner.len(), 3);
    assert_eq!(unsafe { inner[0].name.as_str() }, "class");
    assert_eq!(inner[0].ty.kind, TypeKind::TypeString);
    assert_eq!(unsafe { inner[1].name.as_str() }, "x");
    assert_eq!(unsafe { inner[1].ty.schema.int_ }.bits, 32);
}

/// A setting written from here and read back through the plugin's own call.
fn settings_read_back(descriptor: &mstu_sdk::PluginDescriptor) {
    let handle = (descriptor.create)(std::ptr::null(), Str::new("fixture-1"));

    assert!(!handle.is_null());
    assert_eq!(unsafe { (descriptor.id)(handle).as_str() }, "fixture-1");

    assert!((descriptor.set_parameter)(handle, 0, Value::from(0.75)));
    assert!(!(descriptor.set_parameter)(handle, 7, Value::from(0.75)));

    let mut engine = Engine::default();
    let mut out = writer(&mut engine);

    assert!((descriptor.settings)(handle, &mut out));
    assert_eq!(unsafe { engine.values[0].data.float_ }, 0.75);

    (descriptor.release)(handle);
}

/// One image in, the same image out with a detection beside it.
fn a_message_is_written(descriptor: &mstu_sdk::PluginDescriptor) {
    let handle = (descriptor.create)(std::ptr::null(), Str::new("fixture-2"));

    let image = [1u8, 2, 3, 4, 5];
    let input = [
        Value::from(7u64),
        Value::from(Slice::from_raw_parts(image.as_ptr(), image.len())),
    ];

    let message = Message {
        values: Slice::from_raw_parts(input.as_ptr(), input.len()),
    };

    let mut engine = Engine::default();
    let mut out = writer(&mut engine);

    let mut p_ctx = mstu_sdk::ProcessContext {
        input: &message,
        output: &mut out,
    };

    assert!((descriptor.process)(handle, &mut p_ctx));

    assert_eq!(engine.values.len(), 3);
    assert_eq!(unsafe { engine.values[0].data.uint_ }, 7);
    assert_eq!(engine.values[1].kind, ValueKind::ValueBytes);
    assert_eq!(unsafe { engine.values[1].data.bytes_.as_slice() }, &image);

    // The payload is the memory the writer reserved, not one of the plugin's
    // own: the engine owns every byte a message carries.
    assert_eq!(
        unsafe { engine.values[1].data.bytes_ }.ptr,
        engine.reserved.as_ptr()
    );

    assert_eq!(engine.values[2].kind, ValueKind::ValueList);

    let objects = unsafe { engine.values[2].data.list_.as_slice() };
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].kind, ValueKind::ValueRecord);

    let object = unsafe { objects[0].data.record_.as_slice() };
    assert_eq!(object.len(), 3);
    assert_eq!(unsafe { object[0].data.string_.as_str() }, "person");
    assert_eq!(unsafe { object[1].data.uint_ }, 12);
    assert_eq!(unsafe { object[2].data.uint_ }, 34);

    // Nothing was decoded, so a frame it cannot read is refused rather than
    // guessed at.
    let empty = Message {
        values: Slice::empty(),
    };

    let mut p_ctx = mstu_sdk::ProcessContext {
        input: &empty,
        output: &mut out,
    };

    assert!(!(descriptor.process)(handle, &mut p_ctx));

    (descriptor.release)(handle);
}

#[test]
fn a_plugin_written_in_c_is_built_from_the_header_alone() {
    let Some(tool) = compiler(&["cc", "gcc", "clang"]) else {
        return println!("skipped: no C compiler installed");
    };

    let dir = scratch("c");

    exercise(&build(&tool, "-std=c11", &dir));

    let _ = std::fs::remove_dir_all(&dir);
}

/// The same plugin as C++, which needs the header's `extern "C"`: without it
/// `plugin_descriptor` is mangled and the engine looks for a name that is not
/// there.
#[test]
fn a_plugin_written_in_cpp_exports_a_name_the_engine_can_find() {
    let Some(tool) = compiler(&["c++", "g++", "clang++"]) else {
        return println!("skipped: no C++ compiler installed");
    };

    let dir = scratch("cpp");

    exercise(&build(&tool, "-std=c++20", &dir));

    let _ = std::fs::remove_dir_all(&dir);
}
