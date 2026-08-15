pub mod ast_traits;
pub mod core;
pub mod generated {
    use strum::EnumCount;

    pub mod flags;
    pub mod nodes;
    pub mod syntax_kind;

    const _: () = {
        // While this shouldn't change, it's a good sanity check.
        assert!(syntax_kind::SyntaxKind::COUNT == 353);
    };
}

pub use generated::{
    flags::{NodeFlags, TokenFlags},
    syntax_kind::SyntaxKind,
};
