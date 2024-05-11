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

#[derive(Copy, Clone, Debug)]
pub struct Wrapping {
    current: usize,
    max: usize,
}

impl Wrapping {
    #[must_use]
    pub fn new(current: usize, max: usize) -> Self {
        Self { current, max }
    }

    #[must_use]
    pub fn current(self) -> usize {
        self.current
    }

    #[must_use]
    pub fn inc(mut self) -> Self {
        if self.current >= self.max {
            self.current = 0;
        } else {
            self.current += 1;
        }

        self
    }

    #[must_use]
    pub fn dec(mut self) -> Self {
        if self.current == 0 {
            self.current = self.max;
        } else {
            self.current -= 1;
        }
        self
    }
}
