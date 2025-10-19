#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug)]
pub struct GridMap {
    width: usize,
    height: usize,
    tiles: Vec<TileKind>,
}

impl GridMap {
    pub fn new(width: usize, height: usize, fill: TileKind) -> Self {
        let tiles = vec![fill; width * height];
        Self { width, height, tiles }
    }

    /// Create an empty map filled with grass
    pub fn empty_grass(width: usize, height: usize) -> Self {
        Self::new(width, height, TileKind::Grass)
    }

    /// Create a maze-like map with walls
    pub fn maze_walls(width: usize, height: usize) -> Self {
        let mut m = Self::new(width, height, TileKind::Grass);

        // Add some walls to create a maze-like structure
        // Vertical walls
        m.fill_rect(5, 0, 1, 10, TileKind::Wall);
        m.fill_rect(10, 5, 1, 15, TileKind::Wall);
        m.fill_rect(15, 0, 1, 12, TileKind::Wall);
        m.fill_rect(20, 8, 1, 12, TileKind::Wall);

        // Horizontal walls
        m.fill_rect(0, 5, 8, 1, TileKind::Wall);
        m.fill_rect(12, 10, 8, 1, TileKind::Wall);
        m.fill_rect(5, 15, 10, 1, TileKind::Wall);
        m.fill_rect(18, 20, 6, 1, TileKind::Wall);

        // Add some water features
        m.fill_rect(2, 2, 3, 3, TileKind::Water);
        m.fill_rect(18, 2, 4, 2, TileKind::Water);

        m
    }

    /// Create a desert map with sand and some water
    pub fn desert_oasis(width: usize, height: usize) -> Self {
        let mut m = Self::new(width, height, TileKind::Sand);

        // Add some grass patches
        m.fill_rect(3, 3, 4, 4, TileKind::Grass);
        m.fill_rect(15, 8, 5, 3, TileKind::Grass);
        m.fill_rect(8, 18, 3, 5, TileKind::Grass);

        // Add water oases
        m.fill_rect(5, 5, 2, 2, TileKind::Water);
        m.fill_rect(17, 10, 2, 2, TileKind::Water);
        m.fill_rect(10, 20, 2, 2, TileKind::Water);

        // Add some trees around water
        m.set(4, 4, TileKind::Tree);
        m.set(7, 6, TileKind::Tree);
        m.set(16, 9, TileKind::Tree);
        m.set(19, 11, TileKind::Tree);
        m.set(9, 19, TileKind::Tree);
        m.set(11, 21, TileKind::Tree);

        m
    }

    /// Create a default map with a central lake, a ring of grass around it,
    /// and an outer ring of trees (water -> grass ring -> tree ring -> grass).
    pub fn default_grass_trees_water(width: usize, height: usize) -> Self {
        let mut m = GridMap::new(width, height, TileKind::Grass);

        // Lake: ellipse in the center
        let cx = (width as f32) / 2.0;
        let cy = (height as f32) / 2.0;
        let rx = (width as f32) * 0.25;
        let ry = (height as f32) * 0.2;

        let mut water = vec![false; width * height];
        let idx = |x: usize, y: usize| y * width + x;

        for y in 0..height {
            for x in 0..width {
                let dx = (x as f32) - cx;
                let dy = (y as f32) - cy;
                if (dx * dx) / (rx * rx) + (dy * dy) / (ry * ry) <= 1.0 {
                    water[idx(x, y)] = true;
                    let _ = m.set(x, y, TileKind::Water);
                }
            }
        }

        // Ensure a one-tile ring of grass adjacent to water (already grass by default).
        // Then add an outer ring of trees (chebyshev distance exactly 2 from any water tile)
        for y in 0..height {
            for x in 0..width {
                if water[idx(x, y)] { continue; }

                let mut near1 = false; // within distance 1 of water
                let mut near2 = false; // within distance 2 of water
                'outer: for dy in -2i32..=2 {
                    for dx in -2i32..=2 {
                        if dx == 0 && dy == 0 { continue; }
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        if nx < 0 || ny < 0 { continue; }
                        let (nx, ny) = (nx as usize, ny as usize);
                        if nx >= width || ny >= height { continue; }
                        if water[idx(nx, ny)] {
                            near2 = true;
                            if dx.abs().max(dy.abs()) <= 1 { near1 = true; break 'outer; }
                        }
                    }
                }
                if near2 && !near1 {
                    // distance 2 ring
                    let _ = m.set(x, y, TileKind::Tree);
                } else if near1 {
                    // distance 1 ring is kept grass explicitly
                    let _ = m.set(x, y, TileKind::Grass);
                }
            }
        }

        m
    }

    #[inline]
    pub fn width(&self) -> usize { self.width }

    #[inline]
    pub fn height(&self) -> usize { self.height }

    #[inline]
    pub fn in_bounds(&self, x: usize, y: usize) -> bool {
        x < self.width && y < self.height
    }

    #[inline]
    fn idx(&self, x: usize, y: usize) -> Option<usize> {
        if self.in_bounds(x, y) { Some(y * self.width + x) } else { None }
    }

    pub fn get(&self, x: usize, y: usize) -> Option<&TileKind> {
        self.idx(x, y).map(|i| &self.tiles[i])
    }

    pub fn set(&mut self, x: usize, y: usize, kind: TileKind) -> bool {
        if let Some(i) = self.idx(x, y) {
            self.tiles[i] = kind;
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self, kind: TileKind) {
        self.tiles.fill(kind);
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, kind: TileKind) {
        let x1 = (x + w).min(self.width);
        let y1 = (y + h).min(self.height);
        for yy in y..y1 {
            for xx in x..x1 {
                let _ = self.set(xx, yy, kind.clone());
            }
        }
    }

    pub fn is_traversable(&self, x: usize, y: usize) -> bool {
        self.get(x, y)
            .map(|k| k.is_traversable())
            .unwrap_or(false)
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
