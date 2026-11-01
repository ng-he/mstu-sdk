# mstu-sdk

The ABI an [mstu](https://github.com/ng-he/mstu) plugin is written against.

A plugin is a shared library exporting one symbol, `plugin_descriptor`, which
returns a table of function pointers: what the plugin is called, the schema of
the messages it takes and produces, how to process one, the events it raises
and the commands it answers. The engine owns every byte a message carries, so
a plugin allocates nothing per frame: it either writes into memory the engine
reserves for it, or hands over values the engine copies.

## In Rust

```toml
[dependencies]
mstu-sdk = "0.1"

[lib]
crate-type = ["cdylib"]
```

```rust
use mstu_sdk::{Message, PluginDescriptor, Slice, message};

extern "C" fn process(instance: PluginHandle, ctx: *mut ProcessContext) -> bool {
    let output = unsafe { &mut *(*ctx).output };

    // Engine memory, written in place and never copied.
    let payload = (output.reserve)(output, len);

    let values = [start_time.into(), Slice::from_raw_parts(payload, len).into()];

    (output.fill)(
        output,
        Message { values: Slice::from_raw_parts(values.as_ptr(), values.len()) },
    )
}
```

## In another language

`abi/mstu-sdk.h` is generated from this crate and carries the same two calls,
with helpers for building values:

```c
#include "mstu-sdk.h"

uint8_t *payload = mstu_reserve(output, len);

struct Value values[2] = { mstu_uint(start_time), mstu_bytes(payload, len) };

mstu_fill(output, values, 2);
```

A schema is written the same way, with the types the header names and two
macros for the parts C has no shorthand for:

```c
static const struct Field OBJECT_FIELDS[] = {
  { MSTU_STR("class"), &MSTU_TYPE_STRING, MSTU_STR("What was found.") },
  { MSTU_STR("x"), &MSTU_TYPE_U32, MSTU_STR("Left edge in pixels.") },
};

static const struct Schema OBJECT_SCHEMA = { MSTU_SLICE(OBJECT_FIELDS) };

static const struct Type OBJECT_TYPE = MSTU_TYPE_RECORD(&OBJECT_SCHEMA);
static const struct Type OBJECTS_TYPE = MSTU_TYPE_LIST(&OBJECT_TYPE);
```

Build it as a shared library, export `plugin_descriptor`, and the engine will
load it like any other.

**Nothing is linked against**: the header is the whole of it, and reading a
message goes through `mstu_value` rather than through this crate. A plugin in
C++ gets the same header, whose declarations are wrapped in `extern "C"` so
`plugin_descriptor` keeps the name the engine looks for.

`tests/plugin.c` is a plugin that does all of the above, built and run by
`tests/plugin.rs` with a C compiler and again with a C++ one. It is there so
that a hole in the header fails here rather than in somebody's plugin.

## License

MIT or Apache-2.0, at your option.
