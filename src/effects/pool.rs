/// Something on screen for a while: aged every frame and retired once it is spent.
pub trait Effect {
    /// Move on by `dt` seconds.
    fn advance(&mut self, dt: f32);
    /// Finished, and can be dropped.
    fn done(&self) -> bool;
}

/// A bounded list of live effects, oldest first.
///
/// The bound is the shader's array length: past it the newest pushes out the oldest, which
/// has almost always faded furthest and is the one least missed.
pub struct Pool<T> {
    items: Vec<T>,
    cap: usize,
}

impl<T> Pool<T> {
    pub fn new(cap: usize) -> Self {
        Pool { items: Vec::with_capacity(cap), cap }
    }

    pub fn push(&mut self, item: T) {
        if self.items.len() >= self.cap {
            self.items.remove(0);
        }
        self.items.push(item);
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.items.iter()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl<T: Effect> Pool<T> {
    /// Age everything by `dt` and hand back whatever finished, for the caller to follow up
    /// on or drop. Nothing finishes on most frames, and an empty `Vec` does not allocate.
    pub fn advance(&mut self, dt: f32) -> Vec<T> {
        let mut spent = Vec::new();
        let mut i = 0;
        while i < self.items.len() {
            self.items[i].advance(dt);
            if self.items[i].done() {
                spent.push(self.items.remove(i));
            } else {
                i += 1;
            }
        }
        spent
    }
}

/// A countdown to the next time something happens on its own.
pub struct Cadence {
    left: f32,
}

impl Cadence {
    pub fn after(secs: f32) -> Self {
        Cadence { left: secs }
    }

    /// Count down by `dt`; true once the wait is over. Stays true until `wait` is called.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.left -= dt;
        self.left <= 0.0
    }

    pub fn wait(&mut self, secs: f32) {
        self.left = secs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Timer(f32);
    impl Effect for Timer {
        fn advance(&mut self, dt: f32) {
            self.0 -= dt;
        }
        fn done(&self) -> bool {
            self.0 <= 0.0
        }
    }

    #[test]
    fn full_pool_evicts_the_oldest() {
        let mut p = Pool::new(2);
        for t in [1.0, 2.0, 3.0] {
            p.push(Timer(t));
        }
        assert_eq!(p.iter().map(|t| t.0).collect::<Vec<_>>(), [2.0, 3.0]);
    }

    #[test]
    fn advance_returns_what_finished_and_keeps_order() {
        let mut p = Pool::new(8);
        for t in [0.5, 2.0, 0.2, 3.0] {
            p.push(Timer(t));
        }
        let spent = p.advance(1.0);
        assert_eq!(spent.len(), 2);
        assert_eq!(p.iter().map(|t| t.0).collect::<Vec<_>>(), [1.0, 2.0]);
    }
}
