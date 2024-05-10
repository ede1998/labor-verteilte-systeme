
pub trait ApplyIf: Sized {
    fn apply_if<F: FnOnce(Self) -> Self>(self, condition: bool, f: F) -> Self {
        self.apply_or_else(condition, f, std::convert::identity)
    }
    fn apply_or_else<F1, F2>(self, condition: bool, apply: F1, else_apply: F2) -> Self
    where
        F1: FnOnce(Self) -> Self,
        F2: FnOnce(Self) -> Self;
}

impl<T> ApplyIf for T {
    fn apply_or_else<F1, F2>(self, condition: bool, apply: F1, else_apply: F2) -> Self
    where
        F1: FnOnce(Self) -> Self,
        F2: FnOnce(Self) -> Self,
    {
        if condition {
            apply(self)
        } else {
            else_apply(self)
        }
    }
}
