//! Fixed-capacity player-focus queue and round-robin background cursor.

pub(super) struct Scheduler {
    cursor: usize,
    focus: Vec<usize>,
}
impl Scheduler {
    pub(super) fn new(cell_count: usize) -> Self {
        Self {
            cursor: 0,
            focus: Vec::with_capacity(cell_count.min(4096)),
        }
    }
    pub(super) fn reset(&mut self) {
        self.cursor = 0;
        self.focus.clear();
    }
    pub(super) fn enqueue_focus(&mut self, index: usize) {
        if self.focus.len() < self.focus.capacity() {
            self.focus.push(index);
        }
    }
    pub(super) fn pop_focus(&mut self) -> Option<usize> {
        self.focus.pop()
    }
    pub(super) fn next_background(&mut self, cell_count: usize) -> usize {
        let index = self.cursor;
        self.cursor = (self.cursor + 1) % cell_count;
        index
    }
    pub(super) fn pending_focus(&self) -> usize {
        self.focus.len()
    }
}
