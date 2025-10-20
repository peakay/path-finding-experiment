#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileKind {
    Empty,
    Wall,
    Water,
    Grass,
    Sand,
    Trail,
    Tree,
    Custom(u32),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MapMetadata {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct GridMap {
    pub metadata: Option<MapMetadata>,
    width: usize,
    height: usize,
    tiles: Vec<Vec<TileKind>>,
}

impl<'de> serde::Deserialize<'de> for GridMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct GridMapData {
            name: Option<String>,
            description: Option<String>,
            width: usize,
            height: usize,
            tiles: Vec<Vec<TileKind>>,
        }

        let data = GridMapData::deserialize(deserializer)?;

        let metadata = match (data.name, data.description) {
            (Some(name), Some(description)) => Some(MapMetadata { name, description }),
            _ => None,
        };

        Ok(GridMap {
            metadata,
            width: data.width,
            height: data.height,
            tiles: data.tiles,
        })
    }
}

impl GridMap {
    pub fn new(width: usize, height: usize, fill: TileKind) -> Self {
        let tiles = vec![vec![fill; width]; height];
        Self { metadata: None, width, height, tiles }
    }

    #[inline]
    pub fn width(&self) -> usize { self.width }

    #[inline]
    pub fn height(&self) -> usize { self.height }

    #[inline]
    pub fn in_bounds(&self, x: usize, y: usize) -> bool {
        x < self.width && y < self.height
    }

    pub fn get(&self, x: usize, y: usize) -> Option<&TileKind> {
        self.tiles.get(y).and_then(|row| row.get(x))
    }

    pub fn set(&mut self, x: usize, y: usize, kind: TileKind) -> bool {
        if let Some(row) = self.tiles.get_mut(y) {
            if let Some(tile) = row.get_mut(x) {
                *tile = kind;
                return true;
            }
        }
        false
    }

    pub fn clear(&mut self, kind: TileKind) {
        for row in &mut self.tiles {
            for tile in row {
                *tile = kind;
            }
        }
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, kind: TileKind) {
        let x1 = (x + w).min(self.width);
        let y1 = (y + h).min(self.height);
        for yy in y..y1 {
            for xx in x..x1 {
                if let Some(row) = self.tiles.get_mut(yy) {
                    if let Some(tile) = row.get_mut(xx) {
                        *tile = kind;
                    }
                }
            }
        }
    }

    pub fn is_traversable(&self, x: usize, y: usize) -> bool {
        self.get(x, y)
            .map(|k| k.is_traversable())
            .unwrap_or(false)
    }

    /// Get reference to the tiles grid
    pub fn tiles(&self) -> &Vec<Vec<TileKind>> {
        &self.tiles
    }
}

impl TileKind {
    /// Returns true if this tile can be walked on by agents
    pub fn is_traversable(&self) -> bool {
        matches!(self, TileKind::Empty | TileKind::Grass | TileKind::Sand | TileKind::Trail)
    }

    /// Returns true if this tile blocks movement
    pub fn is_blocking(&self) -> bool {
        !self.is_traversable()
    }

    /// Returns a display name for this tile type
    pub fn name(&self) -> &'static str {
        match self {
            TileKind::Empty => "empty",
            TileKind::Wall => "wall",
            TileKind::Water => "water",
            TileKind::Grass => "grass",
            TileKind::Sand => "sand",
            TileKind::Trail => "grass", // Hide trail from LLM - functionally identical to grass
            TileKind::Tree => "tree",
            TileKind::Custom(_) => "custom",
        }
    }
}
