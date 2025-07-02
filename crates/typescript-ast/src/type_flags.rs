use bitflags::bitflags;

bitflags! {
    #[derive(Default)]
    pub struct TypeFlags: u32 {
        const NONE              = 0;
        const ANY               = 1 << 0;
        const UNKNOWN           = 1 << 1;
        const UNDEFINED         = 1 << 2;
        const NULL              = 1 << 3;
        const VOID              = 1 << 4;
        const STRING            = 1 << 5;
        const NUMBER            = 1 << 6;
        const BIGINT            = 1 << 7;
        const BOOLEAN           = 1 << 8;
        const ES_SYMBOL         = 1 << 9;
        const STRING_LITERAL    = 1 << 10;
        const NUMBER_LITERAL    = 1 << 11;
        const BIGINT_LITERAL    = 1 << 12;
        const BOOLEAN_LITERAL   = 1 << 13;
        const UNIQUE_ES_SYMBOL  = 1 << 14;
        const ENUM_LITERAL      = 1 << 15;
        const ENUM              = 1 << 16;
        const NEVER             = 1 << 17;
        const TYPE_PARAMETER    = 1 << 18;
        const OBJECT            = 1 << 19;
        const UNION             = 1 << 20;
        const INTERSECTION      = 1 << 21;
        const INDEX             = 1 << 22;
        const INDEXED_ACCESS    = 1 << 23;
        const CONDITIONAL       = 1 << 24;
        const SUBSTITUTION      = 1 << 25;
        const NON_PRIMITIVE     = 1 << 26;
        const TEMPLATE_LITERAL  = 1 << 27;
        const STRING_MAPPING    = 1 << 28;
    }
}
