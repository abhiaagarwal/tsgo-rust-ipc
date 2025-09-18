use crate::SyntaxKind;

pub trait AstKind {
    fn kind(&self) -> SyntaxKind;
}

impl<T> From<T> for SyntaxKind
where
    T: AstKind,
{
    fn from(kind: T) -> Self {
        kind.kind()
    }
}
