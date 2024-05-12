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

pub trait HashMapExt {
    type IterItem;
    type KeysItem;
    fn iter_stable(self) -> impl Iterator<Item = Self::IterItem>;
    fn keys_stable(self) -> impl Iterator<Item = Self::KeysItem>;
}

impl<'a, K: std::cmp::Ord, V> HashMapExt for &'a std::collections::HashMap<K, V> {
    type IterItem = (&'a K, &'a V);
    fn iter_stable(self) -> impl Iterator<Item = Self::IterItem> {
        let mut items: Vec<_> = self.iter().collect();
        items.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));
        items.into_iter()
    }

    type KeysItem = &'a K;

    fn keys_stable(self) -> impl Iterator<Item = Self::KeysItem> {
        let mut items: Vec<_> = self.keys().collect();
        items.sort();
        items.into_iter()
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
