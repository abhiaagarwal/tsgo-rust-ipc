pub mod core;
pub mod generated {
    pub mod flags;
    pub mod syntax_kind;

    const _: () = {
        // While this shouldn't change, it's a good sanity check.
        assert!(syntax_kind::SyntaxKind::Count as u16 == 353);
    };
}

pub use generated::{
    flags::{NodeFlags, TokenFlags},
    syntax_kind::SyntaxKind,
};
