use crate::{Slice, Str, Value};

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TypeKind {
    TypeBool,
    TypeInt,
    TypeFloat,
    TypeString,
    TypeRecord,
    TypeEnum,
    TypeList,
    TypeBytes,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BoolSchema {}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IntSchema {
    pub signed: bool,
    pub bits: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FloatSchema {
    pub bits: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StringSchema {}

#[repr(C)]
pub struct EnumVariant {
    pub name: Str,
    pub value: Value,
}

#[repr(C)]
pub struct EnumSchema {
    pub variants: Slice<EnumVariant>,
}

unsafe impl Sync for Slice<EnumVariant> {}
unsafe impl Send for Slice<EnumVariant> {}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RecordSchema {
    pub schema: &'static Schema,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ListSchema {
    pub element: &'static Type,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BytesSchema {}

#[repr(C)]
pub union TypeSchema {
    pub bool_: BoolSchema,
    pub int_: IntSchema,
    pub float_: FloatSchema,
    pub string_: StringSchema,
    pub record_: RecordSchema,
    pub enum_: &'static EnumSchema,
    pub list_: ListSchema,
    pub bytes_: BytesSchema,
}

#[repr(C)]
pub struct Type {
    pub kind: TypeKind,
    pub schema: TypeSchema,
}

unsafe impl Sync for Type {}
unsafe impl Send for Type {}

#[repr(C)]
pub struct Field {
    pub name: Str,
    pub ty: &'static Type,
    pub description: Str,
}

unsafe impl Sync for Field {}
unsafe impl Send for Field {}

#[repr(C)]
pub struct Schema {
    pub fields: Slice<Field>,
}

unsafe impl Sync for Slice<Field> {}
unsafe impl Send for Slice<Field> {}

pub static TYPE_BOOL: Type = Type {
    kind: TypeKind::TypeBool,
    schema: TypeSchema {
        bool_: BoolSchema {},
    },
};

pub static TYPE_I8: Type = Type {
    kind: TypeKind::TypeInt,
    schema: TypeSchema {
        int_: IntSchema {
            signed: true,
            bits: 8,
        },
    },
};

pub static TYPE_U8: Type = Type {
    kind: TypeKind::TypeInt,
    schema: TypeSchema {
        int_: IntSchema {
            signed: false,
            bits: 8,
        },
    },
};

pub static TYPE_I16: Type = Type {
    kind: TypeKind::TypeInt,
    schema: TypeSchema {
        int_: IntSchema {
            signed: true,
            bits: 16,
        },
    },
};

pub static TYPE_U16: Type = Type {
    kind: TypeKind::TypeInt,
    schema: TypeSchema {
        int_: IntSchema {
            signed: false,
            bits: 16,
        },
    },
};

pub static TYPE_I32: Type = Type {
    kind: TypeKind::TypeInt,
    schema: TypeSchema {
        int_: IntSchema {
            signed: true,
            bits: 32,
        },
    },
};

pub static TYPE_U32: Type = Type {
    kind: TypeKind::TypeInt,
    schema: TypeSchema {
        int_: IntSchema {
            signed: false,
            bits: 32,
        },
    },
};

pub static TYPE_I64: Type = Type {
    kind: TypeKind::TypeInt,
    schema: TypeSchema {
        int_: IntSchema {
            signed: true,
            bits: 64,
        },
    },
};

pub static TYPE_U64: Type = Type {
    kind: TypeKind::TypeInt,
    schema: TypeSchema {
        int_: IntSchema {
            signed: false,
            bits: 64,
        },
    },
};

pub static TYPE_F32: Type = Type {
    kind: TypeKind::TypeFloat,
    schema: TypeSchema {
        float_: FloatSchema { bits: 32 },
    },
};

pub static TYPE_F64: Type = Type {
    kind: TypeKind::TypeFloat,
    schema: TypeSchema {
        float_: FloatSchema { bits: 64 },
    },
};

pub static TYPE_STRING: Type = Type {
    kind: TypeKind::TypeString,
    schema: TypeSchema {
        string_: StringSchema {},
    },
};

pub static TYPE_BYTES: Type = Type {
    kind: TypeKind::TypeBytes,
    schema: TypeSchema {
        bytes_: BytesSchema {},
    },
};

#[cfg(test)]
mod tests {
    use super::*;

    /// A field declared as one width and described as another is a bug nobody
    /// sees until the desktop draws the wrong thing, so the table is checked
    /// entry by entry.
    #[test]
    fn every_ready_made_number_says_its_own_width_and_sign() {
        let numbers: [(&Type, bool, u8); 8] = [
            (&TYPE_I8, true, 8),
            (&TYPE_U8, false, 8),
            (&TYPE_I16, true, 16),
            (&TYPE_U16, false, 16),
            (&TYPE_I32, true, 32),
            (&TYPE_U32, false, 32),
            (&TYPE_I64, true, 64),
            (&TYPE_U64, false, 64),
        ];

        for (ty, signed, bits) in numbers {
            assert!(matches!(ty.kind, TypeKind::TypeInt));

            let int = unsafe { ty.schema.int_ };

            assert_eq!(int.signed, signed, "{bits} bit sign");
            assert_eq!(int.bits, bits);
        }
    }

    #[test]
    fn the_floats_say_their_width_too() {
        assert!(matches!(TYPE_F32.kind, TypeKind::TypeFloat));
        assert_eq!(unsafe { TYPE_F32.schema.float_ }.bits, 32);

        assert!(matches!(TYPE_F64.kind, TypeKind::TypeFloat));
        assert_eq!(unsafe { TYPE_F64.schema.float_ }.bits, 64);
    }

    #[test]
    fn the_rest_are_what_they_are_called() {
        assert!(matches!(TYPE_BOOL.kind, TypeKind::TypeBool));
        assert!(matches!(TYPE_STRING.kind, TypeKind::TypeString));
        assert!(matches!(TYPE_BYTES.kind, TypeKind::TypeBytes));
    }

    /// The header gives these numbers to plugins in other languages, so their
    /// order is part of the ABI and cannot be rearranged.
    #[test]
    fn the_type_numbers_are_the_ones_the_header_hands_out() {
        assert_eq!(TypeKind::TypeBool as u32, 0);
        assert_eq!(TypeKind::TypeInt as u32, 1);
        assert_eq!(TypeKind::TypeFloat as u32, 2);
        assert_eq!(TypeKind::TypeString as u32, 3);
        assert_eq!(TypeKind::TypeRecord as u32, 4);
        assert_eq!(TypeKind::TypeEnum as u32, 5);
        assert_eq!(TypeKind::TypeList as u32, 6);
        assert_eq!(TypeKind::TypeBytes as u32, 7);
    }
}
