// A whole plugin in C, built and loaded by plugin.rs.
//
// It is compiled twice, once as C and once as C++, and nothing is linked
// against: the header is all a plugin in another language is handed, so
// anything this file needs that the header does not offer is a hole in it.
//
// The output schema carries a list of records, which is the shape the
// computer-vision plugin needs and the only one nothing had used.

#include "mstu-sdk.h"

#include <stdlib.h>
#include <string.h>

static const struct Metadata METADATA = {
    MSTU_STR("C fixture"),
    MSTU_STR("0.1.0"),
};

static const struct Field INPUT_FIELDS[] = {
    {MSTU_STR("start_time"), &MSTU_TYPE_U64, MSTU_STR("Time of the image.")},
    {MSTU_STR("bytes"), &MSTU_TYPE_BYTES, MSTU_STR("The image itself.")},
};

static const struct Schema INPUT_SCHEMA = {MSTU_SLICE(INPUT_FIELDS)};

// One detection: what it is and where, in the image's own pixels.
static const struct Field OBJECT_FIELDS[] = {
    {MSTU_STR("class"), &MSTU_TYPE_STRING, MSTU_STR("What was found.")},
    {MSTU_STR("x"), &MSTU_TYPE_U32, MSTU_STR("Left edge in pixels.")},
    {MSTU_STR("y"), &MSTU_TYPE_U32, MSTU_STR("Top edge in pixels.")},
};

static const struct Schema OBJECT_SCHEMA = {MSTU_SLICE(OBJECT_FIELDS)};

static const struct Type OBJECT_TYPE = MSTU_TYPE_RECORD(&OBJECT_SCHEMA);
static const struct Type OBJECTS_TYPE = MSTU_TYPE_LIST(&OBJECT_TYPE);

static const struct Field OUTPUT_FIELDS[] = {
    {MSTU_STR("start_time"), &MSTU_TYPE_U64, MSTU_STR("Time of the image.")},
    {MSTU_STR("bytes"), &MSTU_TYPE_BYTES, MSTU_STR("The image, unchanged.")},
    {MSTU_STR("objects"), &OBJECTS_TYPE, MSTU_STR("What was found in it.")},
};

static const struct Schema OUTPUT_SCHEMA = {MSTU_SLICE(OUTPUT_FIELDS)};

static const struct Field SETTINGS_FIELDS[] = {
    {MSTU_STR("confidence"), &MSTU_TYPE_F64, MSTU_STR("How sure is sure enough.")},
};

static const struct Schema SETTINGS_SCHEMA = {MSTU_SLICE(SETTINGS_FIELDS)};

struct Instance {
    struct Str id;
    double confidence;
};

static const struct Metadata *fixture_metadata(void) { return &METADATA; }

static PluginHandle fixture_create(const struct HostContext *ctx, struct Str id) {
    struct Instance *self = (struct Instance *)calloc(1, sizeof(struct Instance));

    (void)ctx;

    if (self != NULL) {
        self->id = id;
        self->confidence = 0.5;
    }

    return self;
}

static struct Str fixture_id(PluginHandle handle) { return ((struct Instance *)handle)->id; }

static void fixture_release(PluginHandle handle) { free(handle); }

static const struct Schema *fixture_input_schema(void) { return &INPUT_SCHEMA; }

static const struct Schema *fixture_output_schema(void) { return &OUTPUT_SCHEMA; }

static const struct Schema *fixture_settings_schema(void) { return &SETTINGS_SCHEMA; }

static const struct Schema *fixture_live_schema(void) { return NULL; }

static bool fixture_settings(PluginHandle handle, struct Writer *output) {
    struct Instance *self = (struct Instance *)handle;
    struct Value values[1];

    values[0] = mstu_float(self->confidence);

    return mstu_fill(output, values, 1);
}

static bool fixture_set_parameter(PluginHandle handle, uintptr_t field, struct Value value) {
    struct Instance *self = (struct Instance *)handle;

    if (field != 0 || value.kind != ValueFloat) {
        return false;
    }

    self->confidence = value.data.float_;

    return true;
}

static struct Str fixture_ui(void) {
    struct Str none = {NULL, 0};

    return none;
}

/// Hands the image back untouched with one detection beside it.
static bool fixture_process(PluginHandle handle, struct ProcessContext *p_ctx) {
    const struct Value *time = mstu_value(p_ctx->input, 0);
    const struct Value *image = mstu_value(p_ctx->input, 1);
    uintptr_t len;
    uint8_t *payload;
    struct Value object[3];
    struct Value objects[1];
    struct Value values[3];

    (void)handle;

    if (time == NULL || image == NULL || image->kind != ValueBytes) {
        return false;
    }

    // Engine memory, so the image is handed on without a copy of our own.
    len = image->data.bytes_.len;
    payload = mstu_reserve(p_ctx->output, len);

    if (payload == NULL) {
        return false;
    }

    memcpy(payload, image->data.bytes_.ptr, len);

    object[0] = mstu_str("person", 6);
    object[1] = mstu_uint(12);
    object[2] = mstu_uint(34);

    objects[0] = mstu_record(object, 3);

    values[0] = mstu_uint(time->data.uint_);
    values[1] = mstu_bytes(payload, len);
    values[2] = mstu_list(objects, 1);

    return mstu_fill(p_ctx->output, values, 3);
}

static struct Slice_EventDescriptor fixture_events(void) {
    struct Slice_EventDescriptor none = {NULL, 0};

    return none;
}

static struct Slice_CommandDescriptor fixture_commands(void) {
    struct Slice_CommandDescriptor none = {NULL, 0};

    return none;
}

static bool fixture_invoke(PluginHandle handle, uintptr_t command, const struct Message *input) {
    (void)handle;
    (void)command;
    (void)input;

    return false;
}

static const struct PluginDescriptor DESCRIPTOR = {
    fixture_metadata,
    fixture_create,
    fixture_id,
    fixture_release,
    fixture_input_schema,
    fixture_output_schema,
    fixture_settings_schema,
    fixture_settings,
    fixture_set_parameter,
    fixture_ui,
    fixture_process,
    fixture_events,
    fixture_commands,
    fixture_invoke,
    fixture_live_schema,
};

// Declared by the header inside its extern "C", which is what keeps this
// symbol findable when the file is compiled as C++.
const struct PluginDescriptor *plugin_descriptor(void) { return &DESCRIPTOR; }
