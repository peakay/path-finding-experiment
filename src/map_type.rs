use crate::map::GridMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapType {
    EmptyGrass,
    MazeWalls,
    DesertOasis,
    LakeTrees,
}

impl MapType {
    pub fn name(&self) -> &'static str {
        match self {
            MapType::EmptyGrass => "Empty Grass",
            MapType::MazeWalls => "Maze Walls",
            MapType::DesertOasis => "Desert Oasis",
            MapType::LakeTrees => "Lake & Trees",
        }
    }

    pub fn create_map(&self, width: usize, height: usize) -> GridMap {
        match self {
            MapType::EmptyGrass => GridMap::empty_grass(width, height),
            MapType::MazeWalls => GridMap::maze_walls(width, height),
            MapType::DesertOasis => GridMap::desert_oasis(width, height),
            MapType::LakeTrees => GridMap::default_grass_trees_water(width, height),
        }
    }
}
