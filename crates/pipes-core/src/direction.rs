/// One of the six axis-aligned directions a pipe can travel through the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

impl Direction {
    pub const ALL: [Direction; 6] = [
        Direction::PosX,
        Direction::NegX,
        Direction::PosY,
        Direction::NegY,
        Direction::PosZ,
        Direction::NegZ,
    ];

    /// The unit offset this direction moves a grid position by.
    pub fn offset(self) -> (i32, i32, i32) {
        match self {
            Direction::PosX => (1, 0, 0),
            Direction::NegX => (-1, 0, 0),
            Direction::PosY => (0, 1, 0),
            Direction::NegY => (0, -1, 0),
            Direction::PosZ => (0, 0, 1),
            Direction::NegZ => (0, 0, -1),
        }
    }

    /// The direction that exactly reverses this one. A pipe never immediately
    /// backtracks into the cell it just came from.
    pub fn opposite(self) -> Direction {
        match self {
            Direction::PosX => Direction::NegX,
            Direction::NegX => Direction::PosX,
            Direction::PosY => Direction::NegY,
            Direction::NegY => Direction::PosY,
            Direction::PosZ => Direction::NegZ,
            Direction::NegZ => Direction::PosZ,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opposite_is_involutive() {
        for dir in Direction::ALL {
            assert_eq!(dir.opposite().opposite(), dir);
        }
    }

    #[test]
    fn opposite_is_never_self() {
        for dir in Direction::ALL {
            assert_ne!(dir.opposite(), dir);
        }
    }

    #[test]
    fn offsets_are_unit_length_and_axis_aligned() {
        for dir in Direction::ALL {
            let (x, y, z) = dir.offset();
            let nonzero = [x, y, z].iter().filter(|v| **v != 0).count();
            assert_eq!(
                nonzero,
                1,
                "direction {dir:?} offset {:?} is not axis-aligned",
                (x, y, z)
            );
            assert_eq!(x.abs() + y.abs() + z.abs(), 1);
        }
    }

    #[test]
    fn opposite_offset_negates() {
        for dir in Direction::ALL {
            let (x, y, z) = dir.offset();
            let (ox, oy, oz) = dir.opposite().offset();
            assert_eq!((ox, oy, oz), (-x, -y, -z));
        }
    }
}
