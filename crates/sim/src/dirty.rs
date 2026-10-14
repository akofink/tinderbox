//! Preallocated, deduplicated dirty texture regions for incremental presentation.
use std::collections::VecDeque;

/// Side length of a square region in the simulation grid.
pub(super) const DIRTY_REGION_SIDE: usize = 16;

/// A non-empty, clipped rectangle of cells needing a texture upload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirtyRegion {
    /// Left edge in cells.
    pub x: usize,
    /// Top edge in cells.
    pub y: usize,
    /// Width in cells, clipped at the world edge.
    pub width: usize,
    /// Height in cells, clipped at the world edge.
    pub height: usize,
}

/// Fixed-capacity FIFO. Repeated changes coalesce until the region is drained.
pub(super) struct DirtyRegions {
    width: usize,
    height: usize,
    chunks_x: usize,
    queued: Vec<bool>,
    order: VecDeque<usize>,
}

impl DirtyRegions {
    pub(super) fn new(width: usize, height: usize) -> Self {
        let chunks_x = width.div_ceil(DIRTY_REGION_SIDE);
        let chunks_y = height.div_ceil(DIRTY_REGION_SIDE);
        let count = chunks_x * chunks_y;
        let order = VecDeque::with_capacity(count);
        Self {
            width,
            height,
            chunks_x,
            queued: vec![false; count],
            order,
        }
    }

    pub(super) fn mark_cell(&mut self, x: usize, y: usize) {
        if x >= self.width || y >= self.height {
            return;
        }
        let chunk_x = x / DIRTY_REGION_SIDE;
        let chunk_y = y / DIRTY_REGION_SIDE;
        let index = chunk_y * self.chunks_x + chunk_x;
        if !self.queued[index] {
            self.queued[index] = true;
            self.order.push_back(index);
        }
    }

    pub(super) fn mark_all(&mut self) {
        for index in 0..self.queued.len() {
            if !self.queued[index] {
                self.queued[index] = true;
                self.order.push_back(index);
            }
        }
    }

    pub(super) fn pop(&mut self) -> Option<DirtyRegion> {
        let index = self.order.pop_front()?;
        self.queued[index] = false;
        let chunk_x = index % self.chunks_x;
        let chunk_y = index / self.chunks_x;
        let x = chunk_x * DIRTY_REGION_SIDE;
        let y = chunk_y * DIRTY_REGION_SIDE;
        Some(DirtyRegion {
            x,
            y,
            width: DIRTY_REGION_SIDE.min(self.width - x),
            height: DIRTY_REGION_SIDE.min(self.height - y),
        })
    }

    pub(super) fn pending_origins(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.order.iter().map(|index| {
            (
                index % self.chunks_x * DIRTY_REGION_SIDE,
                index / self.chunks_x * DIRTY_REGION_SIDE,
            )
        })
    }

    pub(super) fn len(&self) -> usize {
        self.order.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_regions_are_deduplicated_clipped_and_reusable() {
        let mut dirty = DirtyRegions::new(17, 18);
        dirty.mark_cell(16, 17);
        dirty.mark_cell(16, 17);
        dirty.mark_cell(15, 15);
        assert_eq!(dirty.len(), 2);
        assert_eq!(
            dirty.pop(),
            Some(DirtyRegion {
                x: 16,
                y: 16,
                width: 1,
                height: 2,
            })
        );
        dirty.mark_cell(16, 17);
        assert_eq!(dirty.len(), 2);
        assert_eq!(dirty.pop().unwrap().x, 0);
        assert_eq!(dirty.pop().unwrap().x, 16);
        assert_eq!(dirty.pop(), None);
    }

    #[test]
    fn mark_all_keeps_one_entry_per_region() {
        let mut dirty = DirtyRegions::new(17, 18);
        dirty.mark_cell(0, 0);
        dirty.mark_all();
        dirty.mark_all();
        assert_eq!(dirty.len(), 4);
    }
}
