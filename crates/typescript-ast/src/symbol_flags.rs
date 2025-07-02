use bitflags::bitflags;

bitflags! {
    #[derive(Default)]
    pub struct SymbolFlags: u32 {
        const NONE                     = 0;
        const FUNCTION_SCOPED_VARIABLE = 1 << 0;
        const BLOCK_SCOPED_VARIABLE    = 1 << 1;
        const PROPERTY                 = 1 << 2;
        const ENUM_MEMBER              = 1 << 3;
        const FUNCTION                 = 1 << 4;
        const CLASS                    = 1 << 5;
        const INTERFACE                = 1 << 6;
        const CONST_ENUM               = 1 << 7;
        const REGULAR_ENUM             = 1 << 8;
        const VALUE_MODULE             = 1 << 9;
        const NAMESPACE_MODULE         = 1 << 10;
        const TYPE_LITERAL             = 1 << 11;
        const OBJECT_LITERAL           = 1 << 12;
        const METHOD                   = 1 << 13;
        const CONSTRUCTOR              = 1 << 14;
        const GET_ACCESSOR             = 1 << 15;
        const SET_ACCESSOR             = 1 << 16;
        const SIGNATURE                = 1 << 17;
        const TYPE_PARAMETER           = 1 << 18;
        const TYPE_ALIAS               = 1 << 19;
        const EXPORT_VALUE             = 1 << 20;
        const ALIAS                    = 1 << 21;
        const PROTOTYPE                = 1 << 22;
        const EXPORT_STAR              = 1 << 23;
        const OPTIONAL                 = 1 << 24;
        const TRANSIENT                = 1 << 25;
        const ASSIGNMENT               = 1 << 26;
        const MODULE_EXPORTS           = 1 << 27;
        const CONST_ENUM_ONLY_MODULE   = 1 << 28;
        const REPLACEABLE_BY_METHOD    = 1 << 29;
        const GLOBAL_LOOKUP            = 1 << 30;
    }
}
