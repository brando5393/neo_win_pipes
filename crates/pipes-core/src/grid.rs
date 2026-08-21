use std::collections::HashSet;

use crate::direction::Direction;

/// A position on the integer 3D grid the simulation runs on. One grid unit
/// equals one pipe segment length in world space; the renderer scales it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl GridPos {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub fn step(self, dir: Direction) -> GridPos {
        let (dx, dy, dz) = dir.offset();
        GridPos::new(self.x + dx, self.y + dy, self.z + dz)
    }
}

/// The fixed-size box the simulation is contained in. Pipes never leave it;
/// hitting a wall counts the same as hitting another pipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GridBounds {
    pub width: i32,
    pub height: i32,
    pub depth: i32,
}

impl GridBounds {
    pub fn new(width: i32, height: i32, depth: i32) -> Self {
        Self {
            width,
            height,
            depth,
        }
    }

    pub fn contains(self, p: GridPos) -> bool {
        p.x >= 0
            && p.x < self.width
            && p.y >= 0
            && p.y < self.height
            && p.z >= 0
            && p.z < self.depth
    }

    pub fn cell_count(self) -> usize {
        (self.width * self.height * self.depth).max(0) as usize
    }
}

/// Tracks which grid cells are currently filled by a pipe segment or joint.
/// This is what keeps pipes from passing through each other (or themselves).
#[derive(Debug, Clone)]
pub struct OccupancyGrid {
    bounds: GridBounds,
    occupied: HashSet<GridPos>,
}

impl OccupancyGrid {
    pub fn new(bounds: GridBounds) -> Self {
        Self {
            bounds,
            occupied: HashSet::new(),
        }
    }

    pub fn bounds(&self) -> GridBounds {
        self.bounds
    }

    /// A cell is free if it's inside the bounds and nothing occupies it yet.
    pub fn is_free(&self, p: GridPos) -> bool {
        self.bounds.contains(p) && !self.occupied.contains(&p)
    }

    pub fn occupy(&mut self, p: GridPos) {
        debug_assert!(
            self.bounds.contains(p),
            "occupying a cell outside grid bounds"
        );
        self.occupied.insert(p);
    }

    pub fn clear(&mut self) {
        self.occupied.clear();
    }

    pub fn occupied_count(&self) -> usize {
        self.occupied.len()
    }

    /// Fraction of the grid currently filled, in `[0.0, 1.0]`. Used to decide
    /// when the scene is "full enough" to clear and start over, mirroring the
    /// original screensaver's periodic reset.
    pub fn occupancy_ratio(&self) -> f32 {
        let total = self.bounds.cell_count();
        if total == 0 {
            return 1.0;
        }
        self.occupied.len() as f32 / total as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_bounds() -> GridBounds {
        GridBounds::new(4, 4, 4)
    }

    #[test]
    fn step_moves_by_exactly_one_cell() {
        let p = GridPos::new(1, 1, 1);
        assert_eq!(p.step(Direction::PosX), GridPos::new(2, 1, 1));
        assert_eq!(p.step(Direction::NegY), GridPos::new(1, 0, 1));
    }

    #[test]
    fn bounds_contains_only_inside_cells() {
        let b = small_bounds();
        assert!(b.contains(GridPos::new(0, 0, 0)));
        assert!(b.contains(GridPos::new(3, 3, 3)));
        assert!(!b.contains(GridPos::new(4, 0, 0)));
        assert!(!b.contains(GridPos::new(0, -1, 0)));
    }

    #[test]
    fn fresh_grid_is_entirely_free() {
        let grid = OccupancyGrid::new(small_bounds());
        assert!(grid.is_free(GridPos::new(0, 0, 0)));
        assert_eq!(grid.occupancy_ratio(), 0.0);
    }

    #[test]
    fn occupied_cell_is_no_longer_free() {
        let mut grid = OccupancyGrid::new(small_bounds());
        let p = GridPos::new(2, 2, 2);
        grid.occupy(p);
        assert!(!grid.is_free(p));
        assert_eq!(grid.occupied_count(), 1);
    }

    #[test]
    fn out_of_bounds_cell_is_never_free() {
        let grid = OccupancyGrid::new(small_bounds());
        assert!(!grid.is_free(GridPos::new(100, 0, 0)));
    }

    #[test]
    fn clear_resets_occupancy_to_empty() {
        let mut grid = OccupancyGrid::new(small_bounds());
        grid.occupy(GridPos::new(0, 0, 0));
        grid.occupy(GridPos::new(1, 0, 0));
        grid.clear();
        assert_eq!(grid.occupied_count(), 0);
        assert_eq!(grid.occupancy_ratio(), 0.0);
    }

    #[test]
    fn occupancy_ratio_reflects_fraction_filled() {
        let mut grid = OccupancyGrid::new(GridBounds::new(2, 2, 1)); // 4 cells
        grid.occupy(GridPos::new(0, 0, 0));
        assert_eq!(grid.occupancy_ratio(), 0.25);
    }
}
