use bitflags::bitflags;

bitflags! {
    #[derive(Default, Debug, Clone, Copy)]
    pub struct NodeFlags: u32 {
        const NONE = 0;
        // Variable declaration specific
        const LET                               = 1 << 0;  // Variable declaration
        const CONST                             = 1 << 1;  // Variable declaration
        const USING                             = 1 << 2;  // Variable declaration
        // Syntax/parse state flags
        const REPARSED                          = 1 << 3;  // Node was synthesized during parsing
        const SYNTHESIZED                       = 1 << 4;  // Node was synthesized during transformation
        const OPTIONAL_CHAIN                    = 1 << 5;  // Chained MemberExpression rooted to a pseudo-OptionalExpression
        // Contextual flags (populated during binding/checking)
        const EXPORT_CONTEXT                    = 1 << 6;  // Export context (initialized by binding)
        const CONTAINS_THIS                     = 1 << 7;  // Interface contains references to "this"
        const HAS_IMPLICIT_RETURN               = 1 << 8;  // Function implicitly returns on some codepath (initialized by binding)
        const HAS_EXPLICIT_RETURN               = 1 << 9;  // Function has explicit reachable return (initialized by binding)
        // Parsing context flags
        const DISALLOW_IN_CONTEXT               = 1 << 10; // Node parsed in a context where 'in' expressions are not allowed
        const YIELD_CONTEXT                     = 1 << 11; // Node parsed in a 'yield' context of generator
        const DECORATOR_CONTEXT                 = 1 << 12; // Node parsed as part of a decorator
        const AWAIT_CONTEXT                     = 1 << 13; // Node parsed in 'await' context of async function
        const DISALLOW_CONDITIONAL_TYPES         = 1 << 14; // Node parsed where conditional types not allowed
        // Error bookkeeping
        const THIS_NODE_HAS_ERROR               = 1 << 15; // Parser encountered error when creating this node
        const JAVASCRIPT_FILE                   = 1 << 16; // Node parsed in a JavaScript file
        const THIS_NODE_OR_SUBNODES_HAS_ERROR   = 1 << 17; // This node or any child had an error
        const HAS_AGGREGATED_CHILD_DATA         = 1 << 18; // Cached data from children exists in this node
        // Incremental parsing heuristic flags (may never be cleared)
        const POSSIBLY_CONTAINS_DYNAMIC_IMPORT  = 1 << 19;
        const POSSIBLY_CONTAINS_IMPORT_META     = 1 << 20;
        // JSDoc related
        const HAS_JSDOC                         = 1 << 21; // Node has preceding JSDoc
        const JSDOC                             = 1 << 22; // Node parsed inside jsdoc
        // Ambient/with/json contexts
        const AMBIENT                           = 1 << 23; // Node inside ambient context (`declare`)
        const IN_WITH_STATEMENT                 = 1 << 24; // Ancestor is `statement` of with-statement
        const JSON_FILE                         = 1 << 25; // Node parsed in JSON file
        // Deprecation
        const DEPRECATED                        = 1 << 26; // Has '@deprecated' JSDoc tag
        // Additional identifier-specific repurposed flag
        const IDENTIFIER_HAS_EXTENDED_UNICODE_ESCAPE = 1 << 27; // Repurposes CONTAINS_THIS flag in Go implementation

        // Composite flag groups (provided for convenience, match Go sources)
        const BLOCK_SCOPED = Self::LET.bits() | Self::CONST.bits() | Self::USING.bits();
        const CONSTANT     = Self::CONST.bits() | Self::USING.bits();
        const AWAIT_USING  = Self::CONST.bits() | Self::USING.bits(); // identical set as CONSTANT in original source

        // Reachability check flags
        const REACHABILITY_CHECK_FLAGS = Self::HAS_IMPLICIT_RETURN.bits() | Self::HAS_EXPLICIT_RETURN.bits();

        // Context flags
        const CONTEXT_FLAGS = Self::DISALLOW_IN_CONTEXT.bits()
            | Self::DISALLOW_CONDITIONAL_TYPES.bits()
            | Self::YIELD_CONTEXT.bits()
            | Self::DECORATOR_CONTEXT.bits()
            | Self::AWAIT_CONTEXT.bits()
            | Self::JAVASCRIPT_FILE.bits()
            | Self::IN_WITH_STATEMENT.bits()
            | Self::AMBIENT.bits();

        // Excludes when parsing a Type (helpers for parser)
        const TYPE_EXCLUDE_FLAGS = Self::YIELD_CONTEXT.bits() | Self::AWAIT_CONTEXT.bits();

        // Permanent incremental flags
        const PERMANENTLY_SET_INCREMENTAL_FLAGS = Self::POSSIBLY_CONTAINS_DYNAMIC_IMPORT.bits() | Self::POSSIBLY_CONTAINS_IMPORT_META.bits();

        // Identifier-specific
        const IDENTIFIER_HAS_EXTENDED_UNICODE_ESCAPE_REPURPOSE = Self::CONTAINS_THIS.bits(); // Provided for completeness
    }
}
