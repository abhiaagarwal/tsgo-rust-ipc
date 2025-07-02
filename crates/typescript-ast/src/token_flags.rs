use bitflags::bitflags;

bitflags! {
    #[derive(Default, Debug, Clone, Copy)]
    pub struct TokenFlags: u32 {
        const NONE = 0;
        const PRECEDING_LINE_BREAK           = 1 << 0;
        const PRECEDING_JSDOC_COMMENT        = 1 << 1;
        const UNTERMINATED                  = 1 << 2;
        const EXTENDED_UNICODE_ESCAPE       = 1 << 3;
        const SCIENTIFIC                    = 1 << 4;
        const OCTAL                         = 1 << 5;
        const HEX_SPECIFIER                 = 1 << 6;
        const BINARY_SPECIFIER              = 1 << 7;
        const OCTAL_SPECIFIER               = 1 << 8;
        const CONTAINS_SEPARATOR            = 1 << 9;
        const UNICODE_ESCAPE                = 1 << 10;
        const CONTAINS_INVALID_ESCAPE       = 1 << 11;
        const HEX_ESCAPE                    = 1 << 12;
        const CONTAINS_LEADING_ZERO         = 1 << 13;
        const CONTAINS_INVALID_SEPARATOR    = 1 << 14;
        const PRECEDING_JSDOC_LEADING_ASTERISKS = 1 << 15;
    }
}

impl TokenFlags {
    pub const BINARY_OR_OCTAL_SPECIFIER: TokenFlags = TokenFlags::from_bits_truncate(
        Self::BINARY_SPECIFIER.bits() | Self::OCTAL_SPECIFIER.bits(),
    );

    pub const WITH_SPECIFIER: TokenFlags = TokenFlags::from_bits_truncate(
        Self::HEX_SPECIFIER.bits() | Self::BINARY_OR_OCTAL_SPECIFIER.bits(),
    );

    pub const STRING_LITERAL_FLAGS: TokenFlags = TokenFlags::from_bits_truncate(
        Self::HEX_ESCAPE.bits()
            | Self::UNICODE_ESCAPE.bits()
            | Self::EXTENDED_UNICODE_ESCAPE.bits()
            | Self::CONTAINS_INVALID_ESCAPE.bits()
            | Self::CONTAINS_SEPARATOR.bits(),
    );

    pub const NUMERIC_LITERAL_FLAGS: TokenFlags = TokenFlags::from_bits_truncate(
        Self::SCIENTIFIC.bits()
            | Self::OCTAL.bits()
            | Self::CONTAINS_LEADING_ZERO.bits()
            | Self::WITH_SPECIFIER.bits()
            | Self::CONTAINS_SEPARATOR.bits()
            | Self::CONTAINS_INVALID_SEPARATOR.bits(),
    );

    pub const TEMPLATE_LITERAL_LIKE_FLAGS: TokenFlags = TokenFlags::from_bits_truncate(
        Self::HEX_ESCAPE.bits()
            | Self::UNICODE_ESCAPE.bits()
            | Self::EXTENDED_UNICODE_ESCAPE.bits()
            | Self::CONTAINS_INVALID_ESCAPE.bits(),
    );

    pub const IS_INVALID: TokenFlags = TokenFlags::from_bits_truncate(
        Self::OCTAL.bits()
            | Self::CONTAINS_LEADING_ZERO.bits()
            | Self::CONTAINS_INVALID_SEPARATOR.bits()
            | Self::CONTAINS_INVALID_ESCAPE.bits(),
    );
}
