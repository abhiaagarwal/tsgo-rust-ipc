use bitflags::bitflags;

bitflags! {
    #[derive(Default, Debug, Clone, Copy)]
    pub struct ModifierFlags: u32 {
        const NONE = 0;
        // Syntactic / JSDoc modifiers
        const PUBLIC        = 1 << 0;  // Property / Method
        const PRIVATE       = 1 << 1;  // Property / Method
        const PROTECTED     = 1 << 2;  // Property / Method
        const READONLY      = 1 << 3;  // Property / Method
        const OVERRIDE      = 1 << 4;  // Override method
        // Syntactic-only modifiers
        const EXPORT        = 1 << 5;  // Declarations
        const ABSTRACT      = 1 << 6;  // Class / Method / ConstructSignature
        const AMBIENT       = 1 << 7;  // Declarations
        const STATIC        = 1 << 8;  // Property / Method
        const ACCESSOR      = 1 << 9;  // Property
        const ASYNC         = 1 << 10; // Property / Method / Function
        const DEFAULT       = 1 << 11; // Export default declaration
        const CONST         = 1 << 12; // Const enum
        const IN            = 1 << 13; // Contravariance modifier (Type Parameters)
        const OUT           = 1 << 14; // Covariance modifier (Type Parameters)
        const DECORATOR     = 1 << 15; // Contains a decorator
        const IMMEDIATE     = 1 << 16; // Parameter `!` immediate initialization (proposal)
        // JSDoc-only modifiers
        const DEPRECATED        = 1 << 17; // @deprecated tag
        const JSDOC_IMMEDIATE   = 1 << 18; // @param ! immediate initialization (proposal)

        /*
         * The following flags (23-28) are cache-only JSDoc modifiers that parallel
         * the syntactic/JSDoc modifiers above. They should match the order so that
         * shifting by +23 yields the JSDoc cache only counterpart.
         */
        const JSDOC_PUBLIC      = 1 << 23;
        const JSDOC_PRIVATE     = 1 << 24;
        const JSDOC_PROTECTED   = 1 << 25;
        const JSDOC_READONLY    = 1 << 26;
        const JSDOC_OVERRIDE    = 1 << 27;
        const HAS_COMPUTED_JSDOC_MODIFIERS = 1 << 28; // Indicates modifier flags include JSDoc info
        const HAS_COMPUTED_FLAGS           = 1 << 29; // Modifier flags have been computed

        // Composite helper masks
        const ACCESSIBILITY_MODIFIER     = Self::PUBLIC.bits() | Self::PRIVATE.bits() | Self::PROTECTED.bits();
        const PARAMETER_PROPERTY_MODIFIER = Self::ACCESSIBILITY_MODIFIER.bits() | Self::READONLY.bits() | Self::OVERRIDE.bits();
        const NON_PUBLIC_ACCESSIBILITY_MODIFIER = Self::PRIVATE.bits() | Self::PROTECTED.bits();

        const SYNTACTIC_OR_JSDOC_MODIFIERS = Self::PUBLIC.bits() | Self::PRIVATE.bits() | Self::PROTECTED.bits() | Self::READONLY.bits() | Self::OVERRIDE.bits();
        const SYNTACTIC_ONLY_MODIFIERS = Self::EXPORT.bits() | Self::AMBIENT.bits() | Self::ABSTRACT.bits() | Self::STATIC.bits() | Self::ACCESSOR.bits() | Self::ASYNC.bits() | Self::DEFAULT.bits() | Self::CONST.bits() | Self::IN.bits() | Self::OUT.bits() | Self::DECORATOR.bits() | Self::IMMEDIATE.bits();
        const SYNTAX_MODIFIERS = Self::SYNTACTIC_OR_JSDOC_MODIFIERS.bits() | Self::SYNTACTIC_ONLY_MODIFIERS.bits();
        const JSDOC_ONLY_MODIFIERS = Self::DEPRECATED.bits() | Self::JSDOC_IMMEDIATE.bits();
        const JSDOC_CACHE_ONLY_MODIFIERS = Self::JSDOC_PUBLIC.bits() | Self::JSDOC_PRIVATE.bits() | Self::JSDOC_PROTECTED.bits() | Self::JSDOC_READONLY.bits() | Self::JSDOC_OVERRIDE.bits();
        const NON_CACHE_ONLY_MODIFIERS = Self::SYNTACTIC_OR_JSDOC_MODIFIERS.bits() | Self::SYNTACTIC_ONLY_MODIFIERS.bits() | Self::JSDOC_ONLY_MODIFIERS.bits();

        const TYPESCRIPT_MODIFIER = Self::AMBIENT.bits() | Self::PUBLIC.bits() | Self::PRIVATE.bits() | Self::PROTECTED.bits() | Self::READONLY.bits() | Self::ABSTRACT.bits() | Self::CONST.bits() | Self::OVERRIDE.bits() | Self::IN.bits() | Self::OUT.bits() | Self::IMMEDIATE.bits();
        const EXPORT_DEFAULT = Self::EXPORT.bits() | Self::DEFAULT.bits();
        const ALL = Self::EXPORT.bits() | Self::AMBIENT.bits() | Self::PUBLIC.bits() | Self::PRIVATE.bits() | Self::PROTECTED.bits() | Self::STATIC.bits() | Self::READONLY.bits() | Self::ABSTRACT.bits() | Self::ACCESSOR.bits() | Self::ASYNC.bits() | Self::DEFAULT.bits() | Self::CONST.bits() | Self::DEPRECATED.bits() | Self::OVERRIDE.bits() | Self::IN.bits() | Self::OUT.bits() | Self::IMMEDIATE.bits() | Self::DECORATOR.bits();
        const MODIFIER = Self::ALL.bits() & !Self::DECORATOR.bits(); // All except DECORATOR
    }
}
