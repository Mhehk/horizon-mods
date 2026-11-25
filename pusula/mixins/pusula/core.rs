// Pusula — 16.10.1 (mihehk): the map itself, in plain Rust (no game types),
// so all of it is tested. A minimap and a world map in one mod, written from
// scratch; it follows how Xaero's Minimap and World Map behave as their public
// pages describe them, but none of their code, names or images is used.
//
// - `Column` / `Map`: one scanned column (colour, top, water depth), kept per
//   chunk and saved per region (32 x 32 chunks) in a small file of our own.
// - `look` / `scan`: what a block looks like on the map and the top of a
//   column (the surface, or the cave floor under the player).
// - `View` / `cells`: world <-> screen for a map that may turn with the
//   player, and the map drawn as filled cells, runs of one colour merged.
// - `minimap`, `Screen` (the world map), `Waypoint`, `Settings`.
use std::collections::{HashMap, HashSet};

/// A column nobody has seen.
pub const UNKNOWN: i16 = i16::MIN;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Column {
    /// The top block's colour (RGB), the floor's when under water.
    pub rgb: u32,
    /// The top's height: the water's surface when under water.
    pub height: i16,
    /// Blocks of water over the floor (0: dry).
    pub water: u8,
}

impl Column {
    pub const NONE: Column = Column { rgb: 0, height: UNKNOWN, water: 0 };

    pub fn known(&self) -> bool {
        self.height != UNKNOWN
    }
}

/// A chunk's 256 columns, row by row (z, then x).
pub type Cols = Box<[Column; 256]>;

pub fn empty() -> Cols {
    Box::new([Column::NONE; 256])
}

/// A column's place in its chunk.
pub fn index(x: i32, z: i32) -> usize {
    ((z & 15) * 16 + (x & 15)) as usize
}

/// The scanned world: chunks, which regions changed since saved, and a
/// version that moves with every change (the world map's draw cache).
#[derive(Default)]
pub struct Map {
    pub chunks: HashMap<(i32, i32), Cols>,
    pub dirty: HashSet<(i32, i32)>,
    pub version: u64,
}

impl Map {
    pub fn column(&self, x: i32, z: i32) -> Option<Column> {
        let c = self.chunks.get(&(x >> 4, z >> 4))?;
        let col = c[index(x, z)];
        col.known().then_some(col)
    }

    /// A chunk scanned: kept, and its region marked to save, when it changed.
    pub fn put(&mut self, cx: i32, cz: i32, cols: Cols) -> bool {
        if self.chunks.get(&(cx, cz)).is_some_and(|old| **old == *cols) {
            return false;
        }
        self.chunks.insert((cx, cz), cols);
        self.dirty.insert((cx >> 5, cz >> 5));
        self.version += 1;
        true
    }

    /// The colour a column is drawn in, shaded against its northern neighbour.
    pub fn colour(&self, x: i32, z: i32) -> Option<u32> {
        let c = self.column(x, z)?;
        Some(shade(c, self.column(x, z - 1)))
    }

    /// A region's chunks as a file.
    pub fn region(&self, rx: i32, rz: i32) -> Vec<u8> {
        let mut list: Vec<(&(i32, i32), &Cols)> = self.chunks.iter().filter(|(k, _)| k.0 >> 5 == rx && k.1 >> 5 == rz).collect();
        list.sort_by_key(|(k, _)| **k);
        encode(&list)
    }

    /// A region file read back. Chunks scanned this session win over it.
    pub fn load(&mut self, bytes: &[u8]) -> bool {
        let Some(list) = decode(bytes) else { return false };
        for (k, cols) in list {
            self.chunks.entry(k).or_insert(cols);
        }
        self.version += 1;
        true
    }

    pub fn clear(&mut self) {
        self.chunks.clear();
        self.dirty.clear();
        self.version += 1;
    }
}

const MAGIC: &[u8; 4] = b"PSL1";
const COLUMN_BYTES: usize = 6;

/// `PSL1`, a count, then per chunk its x, z and 256 columns of
/// RGB (3 bytes), height (i16) and water (u8), all little-endian.
pub fn encode(list: &[(&(i32, i32), &Cols)]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + list.len() * (8 + 256 * COLUMN_BYTES));
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(list.len() as u32).to_le_bytes());
    for (k, cols) in list {
        out.extend_from_slice(&k.0.to_le_bytes());
        out.extend_from_slice(&k.1.to_le_bytes());
        for c in cols.iter() {
            out.extend_from_slice(&c.rgb.to_le_bytes()[..3]);
            out.extend_from_slice(&c.height.to_le_bytes());
            out.push(c.water);
        }
    }
    out
}

pub fn decode(b: &[u8]) -> Option<Vec<((i32, i32), Cols)>> {
    if b.len() < 8 || &b[..4] != MAGIC {
        return None;
    }
    let n = u32::from_le_bytes(b[4..8].try_into().ok()?) as usize;
    let each = 8 + 256 * COLUMN_BYTES;
    if b.len() != 8 + n * each {
        return None;
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let c = &b[8 + i * each..8 + (i + 1) * each];
        let cx = i32::from_le_bytes(c[0..4].try_into().ok()?);
        let cz = i32::from_le_bytes(c[4..8].try_into().ok()?);
        let mut cols = empty();
        for (j, col) in cols.iter_mut().enumerate() {
            let d = &c[8 + j * COLUMN_BYTES..8 + (j + 1) * COLUMN_BYTES];
            *col = Column {
                rgb: u32::from(d[0]) | u32::from(d[1]) << 8 | u32::from(d[2]) << 16,
                height: i16::from_le_bytes([d[3], d[4]]),
                water: d[5],
            };
        }
        out.push(((cx, cz), cols));
    }
    Some(out)
}

// ---- colours ---------------------------------------------------------------

pub const WATER: u32 = 0x3F76E4;

/// What a block is to the map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Look {
    /// Seen through (air, glass, barriers, light).
    Clear,
    Water,
    /// Drawn in this colour; `roof`: it hides the sky (cave mode), which
    /// leaves and logs do not.
    Block(u32, bool),
}

/// The dyes, as wool, concrete, carpet, terracotta and glass are named.
const DYES: [(&str, u32); 16] = [
    ("white", 0xE9ECEC),
    ("orange", 0xF07613),
    ("magenta", 0xBD44B3),
    ("light_blue", 0x3AAFD9),
    ("yellow", 0xF8C527),
    ("lime", 0x70B919),
    ("pink", 0xED8DAC),
    ("gray", 0x3E4447),
    ("light_gray", 0x8E8E86),
    ("cyan", 0x158991),
    ("purple", 0x792AAC),
    ("blue", 0x35399D),
    ("brown", 0x724728),
    ("green", 0x546D1B),
    ("red", 0xA12722),
    ("black", 0x141519),
];

/// The woods, as planks (and what is made of them) are coloured.
const WOODS: [(&str, u32); 12] = [
    ("oak", 0xA2834F),
    ("spruce", 0x735531),
    ("birch", 0xC0AF79),
    ("jungle", 0xA07351),
    ("acacia", 0xA85A32),
    ("dark_oak", 0x42291A),
    ("mangrove", 0x763631),
    ("cherry", 0xE2B2AC),
    ("pale_oak", 0xE3D9D6),
    ("bamboo", 0xC1AD50),
    ("crimson", 0x653046),
    ("warped", 0x2B6963),
];

/// Blocks by name (without `minecraft:`), roughly their top's average colour.
const BLOCKS: [(&str, u32); 75] = [
    ("terracotta", 0x98614A),
    ("grass_block", 0x6A9F3C),
    ("short_grass", 0x5E8F35),
    ("tall_grass", 0x5E8F35),
    ("fern", 0x58872F),
    ("large_fern", 0x58872F),
    ("dirt", 0x866043),
    ("coarse_dirt", 0x77553B),
    ("rooted_dirt", 0x90684C),
    ("podzol", 0x5B3F18),
    ("mycelium", 0x6F6265),
    ("dirt_path", 0x947A41),
    ("farmland", 0x51341C),
    ("mud", 0x3C393D),
    ("clay", 0xA0A6B3),
    ("gravel", 0x837F7E),
    ("sand", 0xDBCFA3),
    ("red_sand", 0xBE6621),
    ("sandstone", 0xD8CB9B),
    ("red_sandstone", 0xB5621F),
    ("stone", 0x7D7D7D),
    ("cobblestone", 0x7A7A7A),
    ("mossy_cobblestone", 0x6E7A5D),
    ("andesite", 0x888889),
    ("diorite", 0xBCBCBC),
    ("granite", 0x956755),
    ("tuff", 0x6C6D67),
    ("calcite", 0xDFE0DC),
    ("deepslate", 0x505053),
    ("cobbled_deepslate", 0x4D4D50),
    ("bedrock", 0x555555),
    ("obsidian", 0x0F0B19),
    ("snow", 0xF9FEFE),
    ("snow_block", 0xF9FEFE),
    ("powder_snow", 0xF8FDFD),
    ("ice", 0x91B7FD),
    ("packed_ice", 0x8DB4FA),
    ("blue_ice", 0x74A8FD),
    ("moss_block", 0x596E2D),
    ("moss_carpet", 0x596E2D),
    ("lava", 0xD45A12),
    ("magma_block", 0x8E3F1F),
    ("netherrack", 0x612626),
    ("nether_bricks", 0x2C1519),
    ("soul_sand", 0x513E32),
    ("soul_soil", 0x4C3A2F),
    ("basalt", 0x515156),
    ("blackstone", 0x2A2328),
    ("glowstone", 0xAB8654),
    ("crimson_nylium", 0x831F1F),
    ("warped_nylium", 0x2B7265),
    ("nether_wart_block", 0x732A2A),
    ("warped_wart_block", 0x167879),
    ("shroomlight", 0xF09D5C),
    ("end_stone", 0xDBDE9E),
    ("end_stone_bricks", 0xDAE0A2),
    ("purpur_block", 0xA97DA9),
    ("chorus_plant", 0x5D395D),
    ("chorus_flower", 0x977B97),
    ("cactus", 0x587F25),
    ("sugar_cane", 0x94C065),
    ("pumpkin", 0xC67418),
    ("melon", 0x6F9118),
    ("hay_block", 0xA68B0C),
    ("bricks", 0x976253),
    ("stone_bricks", 0x7A7979),
    ("quartz_block", 0xEBE5DE),
    ("iron_block", 0xDCDCDC),
    ("gold_block", 0xF6D03D),
    ("diamond_block", 0x62EDE4),
    ("emerald_block", 0x2ACB57),
    ("lapis_block", 0x1F438C),
    ("redstone_block", 0xAF1805),
    ("coal_block", 0x101010),
    ("copper_block", 0xC06C50),
];

/// The id without its namespace and block state.
fn bare(id: &str) -> &str {
    let id = id.split('[').next().unwrap_or(id);
    id.strip_prefix("minecraft:").unwrap_or(id)
}

/// What a block (`minecraft:oak_stairs[facing=east]` or `oak_stairs`) is to
/// the map. Unknown blocks are grey rather than missing, so nothing is a hole.
pub fn look(id: &str) -> Look {
    let b = bare(id);
    if matches!(b, "air" | "cave_air" | "void_air" | "glass" | "glass_pane" | "tinted_glass" | "barrier" | "light" | "structure_void" | "tripwire")
        || b.ends_with("_glass")
        || b.ends_with("_glass_pane")
    {
        return Look::Clear;
    }
    if matches!(b, "water" | "bubble_column" | "kelp" | "kelp_plant" | "seagrass" | "tall_seagrass") {
        return Look::Water;
    }
    if let Some((_, c)) = BLOCKS.iter().find(|(n, _)| *n == b) {
        return Look::Block(*c, !matches!(b, "short_grass" | "tall_grass" | "fern" | "large_fern" | "sugar_cane"));
    }
    if b.ends_with("_leaves") {
        let c = match b {
            "cherry_leaves" => 0xE9B6CF,
            "birch_leaves" => 0x5D8A3B,
            "spruce_leaves" => 0x3B5E3B,
            "azalea_leaves" | "flowering_azalea_leaves" => 0x5A7332,
            "pale_oak_leaves" => 0x878D76,
            _ => 0x3F7A2B,
        };
        return Look::Block(c, false);
    }
    // `light_gray_wool` does not start with `gray_`, so the first match is
    // the only one.
    if let Some((_, dye)) = DYES.iter().find(|(d, _)| b.starts_with(&format!("{d}_"))) {
        let dull = b.ends_with("_terracotta");
        return Look::Block(if dull { mix(*dye, 0x98614A, 0.45) } else { *dye }, true);
    }
    if let Some((w, c)) = WOODS.iter().filter(|(w, _)| b.starts_with(&format!("{w}_"))).max_by_key(|(w, _)| w.len()) {
        let log = ["_log", "_wood", "_stem", "_hyphae"].iter().any(|s| b.ends_with(s));
        let c = if log {
            match *w {
                "birch" => 0xD8D7D2,
                "crimson" => 0x5C1931,
                "warped" => 0x3A3A4D,
                "cherry" => 0x361D25,
                "pale_oak" => 0x574C49,
                _ => 0x5E4A2E,
            }
        } else {
            *c
        };
        return Look::Block(c, !log);
    }
    if b.ends_with("_ore") {
        return Look::Block(if b.starts_with("deepslate_") { 0x505053 } else if b.starts_with("nether_") { 0x612626 } else { 0x7D7D7D }, true);
    }
    if b.contains("deepslate") {
        return Look::Block(0x505053, true);
    }
    if b.contains("sandstone") {
        return Look::Block(0xD8CB9B, true);
    }
    if b.contains("stone") || b.contains("brick") {
        return Look::Block(0x7D7D7D, true);
    }
    if b.contains("copper") {
        return Look::Block(if b.contains("oxidized") { 0x52A385 } else if b.contains("weathered") { 0x6C9F6A } else if b.contains("exposed") { 0xA17E68 } else { 0xC06C50 }, true);
    }
    if b.contains("flower") || b.contains("tulip") || b.ends_with("_orchid") || matches!(b, "dandelion" | "poppy" | "allium" | "azure_bluet" | "oxeye_daisy" | "cornflower" | "lily_of_the_valley") {
        return Look::Block(0x5E8F35, false);
    }
    Look::Block(0x8A8A8A, true)
}

/// `a` towards `b` by `t` (0: a, 1: b), per channel.
pub fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |s: u32| -> u32 {
        let (x, y) = (((a >> s) & 0xFF) as f32, ((b >> s) & 0xFF) as f32);
        ((x + (y - x) * t).round() as u32) << s
    };
    ch(16) | ch(8) | ch(0)
}

/// Every channel times `f`, kept in 0..=255.
pub fn scale(rgb: u32, f: f32) -> u32 {
    let ch = |s: u32| -> u32 { ((((rgb >> s) & 0xFF) as f32 * f).round().clamp(0.0, 255.0) as u32) << s };
    ch(16) | ch(8) | ch(0)
}

/// A column before shading: the floor seen through its water, which grows
/// bluer and darker the deeper it is.
pub fn base(c: Column) -> u32 {
    if c.water == 0 {
        return c.rgb;
    }
    let t = 0.55 + 0.05 * f32::from(c.water.min(9));
    mix(c.rgb, WATER, t)
}

/// The colour drawn for a column, given its northern neighbour (z - 1), the
/// way a hill is shaded on a map lit from the north.
pub fn shade(c: Column, north: Option<Column>) -> u32 {
    let rgb = base(c);
    // TODO(human): darken or lighten `rgb` by comparing `c.height` with
    // `north`'s. A column higher than its northern neighbour faces the light
    // (lighter), a lower one is in shadow (darker). Use `scale(rgb, f)`.
    rgb
}

/// One block of the column at height `y`, as `scan` sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cell {
    Unloaded,
    Air,
    Water,
    /// A block in this colour.
    Block(u32),
}

/// The deepest water counted; past it the floor is not looked for.
pub const DEEP: u8 = 12;

/// A column from `top` down to `min`: the first block that is not clear,
/// through any water over it.
pub fn scan(top: i32, min: i32, at: &mut dyn FnMut(i32) -> Cell) -> Column {
    let mut water_top: Option<i32> = None;
    let mut depth: u8 = 0;
    let mut y = top;
    while y >= min {
        match at(y) {
            Cell::Unloaded => return Column::NONE,
            // Air, even a pocket under water: the floor is what counts.
            Cell::Air => {}
            Cell::Water => {
                water_top.get_or_insert(y);
                depth = depth.saturating_add(1);
                if depth >= DEEP {
                    return Column { rgb: 0x2A3F8A, height: y_i16(water_top.unwrap_or(y)), water: depth };
                }
            }
            Cell::Block(rgb) => {
                return Column { rgb, height: y_i16(water_top.unwrap_or(y)), water: depth };
            }
        }
        y -= 1;
    }
    match water_top {
        Some(h) => Column { rgb: 0x2A3F8A, height: y_i16(h), water: depth },
        // The void (the End's islands' edges): known, and black.
        None => Column { rgb: 0x000000, height: y_i16(min), water: 0 },
    }
}

/// `scan`, started a little over where the column's top was last time
/// (`hint`), climbing first if something now stands there; without a hint,
/// from `top`. A rescan then reads a few blocks, not the whole sky.
pub fn scan_hinted(top: i32, min: i32, hint: Option<i32>, at: &mut dyn FnMut(i32) -> Cell) -> Column {
    let Some(h) = hint else { return scan(top, min, at) };
    let mut start = (h + 8).clamp(min, top);
    while start < top && !matches!(at(start), Cell::Air) {
        start += 1;
    }
    scan(start, min, at)
}

fn y_i16(y: i32) -> i16 {
    y.clamp(-30000, 30000) as i16
}

/// The lowest and highest block of a dimension.
pub fn heights(dimension: &str) -> (i32, i32) {
    match bare(dimension) {
        "the_nether" => (0, 127),
        "the_end" => (0, 255),
        _ => (-64, 319),
    }
}

// ---- views and drawing -----------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    /// Rect (x, y, w, h) in ARGB.
    Fill([f32; 4], u32),
    /// Text at (x, y) in RGB, with a shadow.
    Text(String, [f32; 2], u32, bool),
}

/// A map on screen: the world point at its middle, how it is turned and how
/// many pixels a block takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub centre: [f64; 2],
    pub yaw: f32,
    pub rotate: bool,
    pub scale: f32,
    pub rect: [f32; 4],
    pub round: bool,
}

impl View {
    pub fn mid(&self) -> [f32; 2] {
        [self.rect[0] + self.rect[2] / 2.0, self.rect[1] + self.rect[3] / 2.0]
    }

    /// The world's direction along screen +x and screen +y, a block long.
    /// Turned, the way the player faces is up (Minecraft's yaw 0 faces +z).
    fn axes(&self) -> ([f64; 2], [f64; 2]) {
        if !self.rotate {
            return ([1.0, 0.0], [0.0, 1.0]);
        }
        let y = f64::from(self.yaw).to_radians();
        let facing = [-y.sin(), y.cos()];
        let right = [-y.cos(), -y.sin()];
        (right, [-facing[0], -facing[1]])
    }

    pub fn world(&self, at: [f32; 2]) -> [f64; 2] {
        let m = self.mid();
        let s = f64::from(self.scale.max(1e-4));
        let d = [f64::from(at[0] - m[0]) / s, f64::from(at[1] - m[1]) / s];
        let (ax, ay) = self.axes();
        [self.centre[0] + ax[0] * d[0] + ay[0] * d[1], self.centre[1] + ax[1] * d[0] + ay[1] * d[1]]
    }

    pub fn screen(&self, w: [f64; 2]) -> [f32; 2] {
        let m = self.mid();
        let d = [w[0] - self.centre[0], w[1] - self.centre[1]];
        let (ax, ay) = self.axes();
        let s = f64::from(self.scale);
        [m[0] + ((d[0] * ax[0] + d[1] * ax[1]) * s) as f32, m[1] + ((d[0] * ay[0] + d[1] * ay[1]) * s) as f32]
    }

    /// Whether a screen point is on the map, `inset` pixels in from its edge.
    pub fn inside(&self, p: [f32; 2], inset: f32) -> bool {
        let m = self.mid();
        if self.round {
            let r = self.rect[2] / 2.0 - inset;
            return (p[0] - m[0]).powi(2) + (p[1] - m[1]).powi(2) <= r * r;
        }
        p[0] >= self.rect[0] + inset && p[0] <= self.rect[0] + self.rect[2] - inset && p[1] >= self.rect[1] + inset && p[1] <= self.rect[1] + self.rect[3] - inset
    }

    /// A screen point pulled onto the map's edge (`inset` in) when outside it.
    pub fn clamp(&self, p: [f32; 2], inset: f32) -> [f32; 2] {
        if self.inside(p, inset) {
            return p;
        }
        let m = self.mid();
        if self.round {
            let r = self.rect[2] / 2.0 - inset;
            let d = [p[0] - m[0], p[1] - m[1]];
            let len = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-6);
            return [m[0] + d[0] / len * r, m[1] + d[1] / len * r];
        }
        [
            p[0].clamp(self.rect[0] + inset, self.rect[0] + self.rect[2] - inset),
            p[1].clamp(self.rect[1] + inset, self.rect[1] + self.rect[3] - inset),
        ]
    }
}

/// The map in `v`, as `cell`-pixel squares each showing the block under its
/// middle; side by side squares of one colour become one fill.
pub fn cells(map: &Map, v: &View, cell: f32, out: &mut Vec<Draw>) {
    let [x0, y0, w, h] = v.rect;
    let cell = cell.max(1.0);
    let (n, rows) = ((w / cell).ceil() as i32, (h / cell).ceil() as i32);
    for j in 0..rows {
        let sy = y0 + j as f32 * cell;
        let ch = cell.min(y0 + h - sy);
        let cy = sy + ch / 2.0;
        let mut run: Option<(f32, f32, u32)> = None;
        let flush = |run: Option<(f32, f32, u32)>, out: &mut Vec<Draw>| {
            if let Some((s, e, c)) = run {
                out.push(Draw::Fill([s, sy, e - s, ch], 0xFF00_0000 | c));
            }
        };
        for i in 0..n {
            let sx = x0 + i as f32 * cell;
            let end = (sx + cell).min(x0 + w);
            let cx = (sx + end) / 2.0;
            let colour = if v.inside([cx, cy], 0.0) {
                let p = v.world([cx, cy]);
                map.colour(p[0].floor() as i32, p[1].floor() as i32)
            } else {
                None
            };
            match (run, colour) {
                (Some((s, e, c)), Some(k)) if c == k && (e - sx).abs() < 0.01 => run = Some((s, end, c)),
                _ => {
                    flush(run, out);
                    run = colour.map(|k| (sx, end, k));
                }
            }
        }
        flush(run, out);
    }
}

/// The map's background: a square, or a disc as one strip per row.
fn backdrop(v: &View, cell: f32, argb: u32, out: &mut Vec<Draw>) {
    if !v.round {
        out.push(Draw::Fill(v.rect, argb));
        return;
    }
    let [x0, y0, w, h] = v.rect;
    let r = w / 2.0;
    let mut y = y0;
    while y < y0 + h {
        let ch = cell.min(y0 + h - y);
        let dy = (y + ch / 2.0) - (y0 + h / 2.0);
        if dy.abs() <= r {
            let half = (r * r - dy * dy).sqrt();
            out.push(Draw::Fill([x0 + r - half, y, 2.0 * half, ch], argb));
        }
        y += ch;
    }
}

/// What the radar shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Player,
    Hostile,
    Other,
}

const HOSTILE: [&str; 36] = [
    "zombie", "husk", "drowned", "zombie_villager", "skeleton", "stray", "bogged", "wither_skeleton", "creeper", "spider", "cave_spider",
    "enderman", "witch", "slime", "magma_cube", "blaze", "ghast", "phantom", "pillager", "vindicator", "evoker", "ravager", "vex",
    "guardian", "elder_guardian", "shulker", "silverfish", "endermite", "hoglin", "zoglin", "piglin_brute", "warden", "breeze", "creaking",
    "ender_dragon", "wither",
];

pub fn kind(id: &str) -> Kind {
    let b = bare(id);
    if b == "player" {
        Kind::Player
    } else if HOSTILE.contains(&b) {
        Kind::Hostile
    } else {
        Kind::Other
    }
}

/// Someone on the radar: where they stand (x, z), what they are, a name.
#[derive(Clone, Debug, PartialEq)]
pub struct Dot {
    pub at: [f64; 3],
    pub kind: Kind,
    pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Waypoint {
    pub name: String,
    pub at: [i32; 3],
    pub colour: u32,
    pub dimension: String,
    /// The last death's (replaced by the next one).
    pub death: bool,
}

impl Waypoint {
    /// The letter on its marker.
    pub fn initial(&self) -> String {
        self.name.chars().find(|c| !c.is_whitespace()).map_or_else(|| "?".to_owned(), |c| c.to_uppercase().collect())
    }

    pub fn xz(&self) -> [f64; 2] {
        [f64::from(self.at[0]) + 0.5, f64::from(self.at[2]) + 0.5]
    }
}

/// Waypoint colours: the sixteen chat colours, dark ones last.
pub const COLOURS: [u32; 16] = [
    0xFF5555, 0x55FF55, 0x5555FF, 0xFFFF55, 0xFF55FF, 0x55FFFF, 0xFFAA00, 0xFFFFFF, 0xAA0000, 0x00AA00, 0x0000AA, 0xAAAA00, 0xAA00AA, 0x00AAAA, 0xAAAAAA,
    0x555555,
];

fn clean(s: &str) -> String {
    s.chars().map(|c| if c == '\t' || c == '\n' || c == '\r' { ' ' } else { c }).collect()
}

/// One waypoint a line: name, x, y, z, colour, dimension, `d` for a death.
pub fn write_waypoints(list: &[Waypoint]) -> String {
    let mut out = String::new();
    for w in list {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{:06X}\t{}\t{}\n",
            clean(&w.name),
            w.at[0],
            w.at[1],
            w.at[2],
            w.colour & 0xFF_FFFF,
            clean(&w.dimension),
            if w.death { "d" } else { "-" }
        ));
    }
    out
}

pub fn read_waypoints(text: &str) -> Vec<Waypoint> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() < 7 {
                return None;
            }
            Some(Waypoint {
                name: f[0].to_owned(),
                at: [f[1].parse().ok()?, f[2].parse().ok()?, f[3].parse().ok()?],
                colour: u32::from_str_radix(f[4], 16).ok()?,
                dimension: f[5].to_owned(),
                death: f[6] == "d",
            })
        })
        .collect()
}

/// A world's folder name: letters, digits, `.`, `-`, `_`; the rest `_`.
pub fn folder(name: &str) -> String {
    let s: String = name.trim().to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' }).collect();
    if s.is_empty() || s.chars().all(|c| c == '.') {
        "_".to_owned()
    } else {
        s
    }
}

/// A world point on screen through the camera (position, yaw, pitch,
/// vertical field of view), the inverse of Eksen's `mouse_ray`. None when
/// behind the camera.
pub fn project(cam: [f64; 3], yaw: f32, pitch: f32, fov: f32, size: [f32; 2], p: [f64; 3]) -> Option<[f32; 2]> {
    let (y, pt) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    let f = [-y.sin() * pt.cos(), -pt.sin(), y.cos() * pt.cos()];
    let r = [-y.cos(), 0.0, -y.sin()];
    let u = [r[1] * f[2] - r[2] * f[1], r[2] * f[0] - r[0] * f[2], r[0] * f[1] - r[1] * f[0]];
    let v = [p[0] - cam[0], p[1] - cam[1], p[2] - cam[2]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let z = dot(v, f);
    if z <= 0.05 {
        return None;
    }
    let t = (f64::from(fov).to_radians() / 2.0).tan();
    let aspect = f64::from(size[0]) / f64::from(size[1].max(1.0));
    let nx = dot(v, r) / (z * t * aspect);
    let ny = dot(v, u) / (z * t);
    Some([((nx + 1.0) / 2.0 * f64::from(size[0])) as f32, ((1.0 - ny) / 2.0 * f64::from(size[1])) as f32])
}

// ---- settings ----------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cave {
    Auto,
    On,
    Off,
}

/// The minimap's zooms: pixels a block.
pub const ZOOMS: [f32; 6] = [0.5, 1.0, 1.5, 2.0, 3.0, 4.0];
/// The world map's zooms.
pub const MAP_ZOOMS: [f32; 9] = [0.0625, 0.125, 0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 8.0];

/// Setting, default, what it is (`/pusula ayar`).
pub const OPTIONS: [(&str, &str, &str); 13] = [
    ("minimap", "acik", "minimap açık/kapalı"),
    ("size", "100", "minimap boyu (piksel, 50-200)"),
    ("zoom", "1", "piksel/blok: 0.5, 1, 1.5, 2, 3, 4 (=/- tuşları)"),
    ("shape", "kare", "kare / daire"),
    ("rotate", "acik", "harita senin baktığın yöne döner"),
    ("corner", "sol-ust", "sol-ust / sag-ust / sol-alt / sag-alt"),
    ("coords", "acik", "minimapin altında koordinatlar"),
    ("radar", "acik", "oyuncular ve moblar noktalarla"),
    ("names", "acik", "oyuncu adları"),
    ("cave", "oto", "mağara modu: oto / acik / kapali"),
    ("in_world", "acik", "waypointler dünyada (ad ve uzaklık)"),
    ("detail", "80", "minimapin bir kenarındaki kare sayısı (32-128)"),
    ("speed", "2", "her karede taranan chunk (1-16)"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub minimap: bool,
    pub size: f32,
    pub zoom: f32,
    pub round: bool,
    pub rotate: bool,
    pub corner: Corner,
    pub coords: bool,
    pub radar: bool,
    pub names: bool,
    pub cave: Cave,
    pub in_world: bool,
    pub detail: u32,
    pub speed: u32,
}

fn yes(v: &str) -> Option<bool> {
    match v {
        "acik" | "açık" | "true" | "on" | "evet" => Some(true),
        "kapali" | "kapalı" | "false" | "off" | "hayir" | "hayır" => Some(false),
        _ => None,
    }
}

/// A setting's value checked (and written the way it is stored).
pub fn checked(key: &str, value: &str) -> Result<String, String> {
    let v = value.trim().to_lowercase();
    let number = |lo: f32, hi: f32| -> Result<String, String> {
        match v.replace(',', ".").parse::<f32>() {
            Ok(n) if (lo..=hi).contains(&n) => Ok(format!("{n}")),
            _ => Err(format!("{key}: {lo} ile {hi} arasında bir sayı yaz.")),
        }
    };
    match key {
        "minimap" | "rotate" | "coords" | "radar" | "names" | "in_world" => match yes(&v) {
            Some(b) => Ok(if b { "acik" } else { "kapali" }.to_owned()),
            None => Err(format!("{key}: acik ya da kapali yaz.")),
        },
        "size" => number(50.0, 200.0),
        "detail" => number(32.0, 128.0).map(|s| format!("{}", s.parse::<f32>().unwrap_or(80.0).round())),
        "speed" => number(1.0, 16.0).map(|s| format!("{}", s.parse::<f32>().unwrap_or(2.0).round())),
        "zoom" => match v.replace(',', ".").parse::<f32>() {
            Ok(n) if ZOOMS.contains(&n) => Ok(format!("{n}")),
            _ => Err("zoom: 0.5, 1, 1.5, 2, 3 ya da 4 yaz.".to_owned()),
        },
        "shape" => match v.as_str() {
            "kare" | "square" => Ok("kare".into()),
            "daire" | "yuvarlak" | "round" | "circle" => Ok("daire".into()),
            _ => Err("shape: kare ya da daire yaz.".into()),
        },
        "corner" => match v.as_str() {
            "sol-ust" | "sol-üst" | "top-left" => Ok("sol-ust".into()),
            "sag-ust" | "sağ-üst" | "top-right" => Ok("sag-ust".into()),
            "sol-alt" | "bottom-left" => Ok("sol-alt".into()),
            "sag-alt" | "sağ-alt" | "bottom-right" => Ok("sag-alt".into()),
            _ => Err("corner: sol-ust, sag-ust, sol-alt ya da sag-alt yaz.".into()),
        },
        "cave" => match v.as_str() {
            "oto" | "auto" => Ok("oto".into()),
            _ => match yes(&v) {
                Some(b) => Ok(if b { "acik" } else { "kapali" }.to_owned()),
                None => Err("cave: oto, acik ya da kapali yaz.".into()),
            },
        },
        _ => Err(format!("Bilinmeyen ayar: {key}. /pusula ayar listeyi gösterir.")),
    }
}

impl Settings {
    pub fn read(get: &dyn Fn(&str) -> Option<String>) -> Settings {
        let raw = |k: &str| -> String {
            let d = OPTIONS.iter().find(|o| o.0 == k).map_or("", |o| o.1);
            get(k).and_then(|v| checked(k, &v).ok()).unwrap_or_else(|| d.to_owned())
        };
        let on = |k: &str| raw(k) == "acik";
        let num = |k: &str| raw(k).parse::<f32>().unwrap_or(0.0);
        Settings {
            minimap: on("minimap"),
            size: num("size"),
            zoom: num("zoom"),
            round: raw("shape") == "daire",
            rotate: on("rotate"),
            corner: match raw("corner").as_str() {
                "sag-ust" => Corner::TopRight,
                "sol-alt" => Corner::BottomLeft,
                "sag-alt" => Corner::BottomRight,
                _ => Corner::TopLeft,
            },
            coords: on("coords"),
            radar: on("radar"),
            names: on("names"),
            cave: match raw("cave").as_str() {
                "acik" => Cave::On,
                "kapali" => Cave::Off,
                _ => Cave::Auto,
            },
            in_world: on("in_world"),
            detail: num("detail") as u32,
            speed: num("speed") as u32,
        }
    }
}

/// The next (or previous) zoom from `now` in `list`.
pub fn step_zoom(list: &[f32], now: f32, up: bool) -> f32 {
    let i = list.iter().position(|z| (*z - now).abs() < 1e-4).unwrap_or(list.len() / 2);
    let j = if up { (i + 1).min(list.len() - 1) } else { i.saturating_sub(1) };
    list[j]
}

// ---- the minimap ---------------------------------------------------------------

pub const FONT: f32 = 9.0;

/// What the minimap needs to know this frame.
pub struct Mini<'a> {
    pub settings: &'a Settings,
    pub map: &'a Map,
    /// The player's feet and yaw.
    pub me: [f64; 3],
    pub yaw: f32,
    pub dots: &'a [Dot],
    pub waypoints: &'a [&'a Waypoint],
    pub screen: [f32; 2],
    pub cave: bool,
}

/// Where the minimap goes on a `screen`.
pub fn minimap_rect(s: &Settings, screen: [f32; 2]) -> [f32; 4] {
    let (m, size) = (5.0, s.size);
    let x = match s.corner {
        Corner::TopLeft | Corner::BottomLeft => m,
        _ => screen[0] - m - size,
    };
    let below = if s.coords { FONT + 3.0 } else { 0.0 };
    let y = match s.corner {
        Corner::TopLeft | Corner::TopRight => m,
        _ => screen[1] - m - size - below,
    };
    [x, y, size, size]
}

fn centred(text: &str, at: [f32; 2], rgb: u32, measure: &dyn Fn(&str) -> f32, out: &mut Vec<Draw>) {
    out.push(Draw::Text(text.to_owned(), [at[0] - measure(text) / 2.0, at[1] - FONT / 2.0 + 1.0], rgb, true));
}

/// A waypoint's square marker with its letter.
fn marker(w: &Waypoint, p: [f32; 2], measure: &dyn Fn(&str) -> f32, out: &mut Vec<Draw>) {
    let half = 5.0;
    out.push(Draw::Fill([p[0] - half - 1.0, p[1] - half - 1.0, 2.0 * half + 2.0, 2.0 * half + 2.0], 0xFF00_0000));
    out.push(Draw::Fill([p[0] - half, p[1] - half, 2.0 * half, 2.0 * half], 0xFF00_0000 | w.colour));
    // Black on light colours, white on dark ones.
    let light = (((w.colour >> 16) & 0xFF) * 299 + ((w.colour >> 8) & 0xFF) * 587 + (w.colour & 0xFF) * 114) / 1000 > 150;
    let text = w.initial();
    out.push(Draw::Text(text.clone(), [p[0] - measure(&text) / 2.0 + 0.5, p[1] - 3.5], if light { 0x000000 } else { 0xFFFFFF }, false));
}

/// The player's arrow: a white dot, and a short line the way they face.
fn arrow(at: [f32; 2], dir: [f32; 2], out: &mut Vec<Draw>) {
    out.push(Draw::Fill([at[0] - 2.5, at[1] - 2.5, 5.0, 5.0], 0xFF00_0000));
    out.push(Draw::Fill([at[0] - 1.5, at[1] - 1.5, 3.0, 3.0], 0xFFFF_FFFF));
    for k in [3.5_f32, 5.0, 6.5] {
        let p = [at[0] + dir[0] * k, at[1] + dir[1] * k];
        out.push(Draw::Fill([p[0] - 1.0, p[1] - 1.0, 2.0, 2.0], 0xFFFF_FFFF));
    }
}

fn dot_colour(k: Kind) -> u32 {
    match k {
        Kind::Player => 0xFFFF_FFFF,
        Kind::Hostile => 0xFFFF_4040,
        Kind::Other => 0xFFFF_E040,
    }
}

pub fn minimap(m: &Mini, measure: &dyn Fn(&str) -> f32) -> Vec<Draw> {
    let s = m.settings;
    let rect = minimap_rect(s, m.screen);
    let v = View { centre: [m.me[0], m.me[2]], yaw: m.yaw, rotate: s.rotate, scale: s.zoom, rect, round: s.round };
    let cell = (s.size / s.detail.max(8) as f32).max(1.0);
    let mut out = Vec::new();
    // A frame one pixel wider all round, then the unexplored dark, the map.
    let frame = View { rect: [rect[0] - 1.0, rect[1] - 1.0, rect[2] + 2.0, rect[3] + 2.0], ..v };
    backdrop(&frame, 1.0, 0xFF00_0000, &mut out);
    backdrop(&v, cell, 0xFF1A_1A1A, &mut out);
    cells(m.map, &v, cell, &mut out);

    if s.radar {
        for d in m.dots {
            let p = v.screen([d.at[0], d.at[2]]);
            if !v.inside(p, 2.0) {
                continue;
            }
            let big = if d.kind == Kind::Player { 2.0 } else { 1.5 };
            out.push(Draw::Fill([p[0] - big - 0.5, p[1] - big - 0.5, 2.0 * big + 1.0, 2.0 * big + 1.0], 0xFF00_0000));
            out.push(Draw::Fill([p[0] - big, p[1] - big, 2.0 * big, 2.0 * big], dot_colour(d.kind)));
            if s.names && d.kind == Kind::Player {
                if let Some(n) = &d.name {
                    centred(n, [p[0], p[1] - 7.0], 0xFFFFFF, measure, &mut out);
                }
            }
        }
    }
    for w in m.waypoints {
        let p = v.clamp(v.screen(w.xz()), 6.0);
        marker(w, p, measure, &mut out);
    }
    // The compass letters on the edge.
    for (label, d) in [("N", [0.0, -1.0]), ("E", [1.0, 0.0]), ("S", [0.0, 1.0]), ("W", [-1.0, 0.0])] {
        if label != "N" && !s.rotate {
            continue;
        }
        let far = v.screen([v.centre[0] + d[0] * 1e5, v.centre[1] + d[1] * 1e5]);
        let p = v.clamp(far, 5.0);
        centred(label, p, if label == "N" { 0xFF5555 } else { 0xDDDDDD }, measure, &mut out);
    }
    let mid = v.mid();
    let dir = if s.rotate {
        [0.0, -1.0]
    } else {
        let y = f64::from(m.yaw).to_radians();
        [(-y.sin()) as f32, (y.cos()) as f32]
    };
    arrow(mid, dir, &mut out);
    if m.cave {
        out.push(Draw::Text("Mağara".into(), [rect[0] + 2.0, rect[1] + 2.0], 0xAAAAAA, true));
    }
    if s.coords {
        let text = format!("{}, {}, {}", m.me[0].floor() as i64, m.me[1].floor() as i64, m.me[2].floor() as i64);
        centred(&text, [mid[0], rect[1] + rect[3] + 3.0 + FONT / 2.0], 0xFFFFFF, measure, &mut out);
    }
    out
}

/// A waypoint in the world: its square, name and distance.
pub fn label(w: &Waypoint, at: [f32; 2], metres: f64, measure: &dyn Fn(&str) -> f32) -> Vec<Draw> {
    let mut out = Vec::new();
    marker(w, at, measure, &mut out);
    let text = format!("{} ({} m)", w.name, metres.round() as i64);
    let tw = measure(&text);
    out.push(Draw::Fill([at[0] - tw / 2.0 - 2.0, at[1] + 8.0, tw + 4.0, FONT + 2.0], 0x8000_0000));
    out.push(Draw::Text(text, [at[0] - tw / 2.0, at[1] + 9.0], 0xFFFFFF, true));
    out
}

// ---- the world map screen ------------------------------------------------------

/// The new waypoint dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct Dialog {
    pub name: String,
    pub at: [i32; 3],
    pub colour: usize,
}

/// What the screen asks the game to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Nothing,
    /// A chat command to send.
    Command(String),
    /// Save a waypoint: name, where, colour.
    Save(String, [i32; 3], u32),
    /// Remove the n-th waypoint of the list shown.
    Delete(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Screen {
    pub centre: [f64; 2],
    pub scale: f32,
    /// The map follows the player until dragged.
    pub follow: bool,
    /// Dragging: where the pointer went down and the centre then.
    pub drag: Option<([f32; 2], [f64; 2])>,
    /// The right-click menu: where it opened and the block under it.
    pub menu: Option<([f32; 2], [i32; 2])>,
    pub dialog: Option<Dialog>,
    pub list: bool,
    pub size: [f32; 2],
    pub open: bool,
    pub close_asked: bool,
}

impl Default for Screen {
    fn default() -> Self {
        Screen { centre: [0.0, 0.0], scale: 1.0, follow: true, drag: None, menu: None, dialog: None, list: true, size: [480.0, 270.0], open: false, close_asked: false }
    }
}

const BAR: f32 = 18.0;
const LIST_W: f32 = 140.0;
const ROW: f32 = 12.0;
const MENU_W: f32 = 112.0;
const MENU_ITEMS: [&str; 2] = ["Waypoint oluştur", "Işınlan (/tp)"];

fn hit(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[0] < r[0] + r[2] && p[1] >= r[1] && p[1] < r[1] + r[3]
}

impl Screen {
    pub fn view(&self) -> View {
        View { centre: self.centre, yaw: 0.0, rotate: false, scale: self.scale, rect: [0.0, 0.0, self.size[0], self.size[1]], round: false }
    }

    fn follow_button(&self) -> [f32; 4] {
        [self.size[0] - 186.0, 2.0, 90.0, 14.0]
    }

    fn list_button(&self) -> [f32; 4] {
        [self.size[0] - 92.0, 2.0, 90.0, 14.0]
    }

    fn list_rect(&self) -> [f32; 4] {
        [self.size[0] - LIST_W - 2.0, BAR + 2.0, LIST_W, self.size[1] - BAR - 30.0]
    }

    fn row(&self, i: usize) -> [f32; 4] {
        let l = self.list_rect();
        [l[0] + 2.0, l[1] + 14.0 + i as f32 * ROW, l[2] - 4.0, ROW]
    }

    fn rows(&self) -> usize {
        ((self.list_rect()[3] - 16.0) / ROW).max(0.0) as usize
    }

    fn menu_item(&self, at: [f32; 2], i: usize) -> [f32; 4] {
        let x = at[0].min(self.size[0] - MENU_W - 2.0);
        let y = at[1].min(self.size[1] - 2.0 * 14.0 - 2.0);
        [x, y + i as f32 * 14.0, MENU_W, 14.0]
    }

    fn dialog_rect(&self) -> [f32; 4] {
        let (w, h) = (210.0, 96.0);
        [(self.size[0] - w) / 2.0, (self.size[1] - h) / 2.0, w, h]
    }

    fn dialog_parts(&self) -> ([f32; 4], [f32; 4], [f32; 4], [f32; 4]) {
        let d = self.dialog_rect();
        let field = [d[0] + 8.0, d[1] + 22.0, d[2] - 16.0, 14.0];
        let swatch = [d[0] + 8.0, d[1] + 44.0, 14.0, 14.0];
        let save = [d[0] + 8.0, d[1] + d[3] - 22.0, (d[2] - 24.0) / 2.0, 16.0];
        let cancel = [save[0] + save[2] + 8.0, save[1], save[2], 16.0];
        (field, swatch, save, cancel)
    }

    /// The map opened: on the player, or on a new waypoint dialog there.
    pub fn opened(&mut self, me: [f64; 3], new_waypoint: bool, count: usize) {
        self.centre = [me[0], me[2]];
        self.follow = true;
        self.drag = None;
        self.menu = None;
        self.dialog = new_waypoint.then(|| Dialog {
            name: format!("Waypoint {}", count + 1),
            at: me.map(|v| v.floor() as i32),
            colour: count % COLOURS.len(),
        });
    }

    /// A button down. `list` is the waypoints shown in the list (this
    /// dimension), `height` a column's top for teleporting.
    pub fn press(&mut self, at: [f32; 2], button: i32, list: &[&Waypoint], height: &dyn Fn(i32, i32) -> Option<i32>) -> Action {
        if self.dialog.is_some() {
            let (_, swatch, save, cancel) = self.dialog_parts();
            if hit(swatch, at) {
                if let Some(d) = self.dialog.as_mut() {
                    // Left: the next colour; right: the one before.
                    d.colour = (d.colour + if button == 1 { COLOURS.len() - 1 } else { 1 }) % COLOURS.len();
                }
                return Action::Nothing;
            }
            if hit(save, at) {
                return self.save();
            }
            if hit(cancel, at) {
                self.dialog = None;
            }
            return Action::Nothing;
        }
        if let Some((m, block)) = self.menu.take() {
            if button == 0 {
                if hit(self.menu_item(m, 0), at) {
                    let y = height(block[0], block[1]).map_or(64, |h| h + 1);
                    self.dialog = Some(Dialog { name: format!("Waypoint {}", list.len() + 1), at: [block[0], y, block[1]], colour: list.len() % COLOURS.len() });
                    return Action::Nothing;
                }
                if hit(self.menu_item(m, 1), at) {
                    let y = height(block[0], block[1]).map_or_else(|| "~".to_owned(), |h| (h + 1).to_string());
                    return Action::Command(format!("/tp @s {} {y} {}", block[0], block[1]));
                }
            }
            return Action::Nothing;
        }
        if hit(self.follow_button(), at) {
            self.follow = true;
            return Action::Nothing;
        }
        if hit(self.list_button(), at) {
            self.list = !self.list;
            return Action::Nothing;
        }
        if self.list && hit(self.list_rect(), at) {
            for (i, w) in list.iter().enumerate().take(self.rows()) {
                let r = self.row(i);
                if hit(r, at) {
                    if at[0] >= r[0] + r[2] - 10.0 {
                        return Action::Delete(i);
                    }
                    self.centre = w.xz();
                    self.follow = false;
                    return Action::Nothing;
                }
            }
            return Action::Nothing;
        }
        match button {
            0 => self.drag = Some((at, self.centre)),
            1 => {
                let p = self.view().world(at);
                self.menu = Some((at, [p[0].floor() as i32, p[1].floor() as i32]));
            }
            _ => {}
        }
        Action::Nothing
    }

    pub fn dragged(&mut self, at: [f32; 2]) {
        if let Some((from, centre)) = self.drag {
            let s = f64::from(self.scale);
            self.centre = [centre[0] - f64::from(at[0] - from[0]) / s, centre[1] - f64::from(at[1] - from[1]) / s];
            self.follow = false;
        }
    }

    pub fn released(&mut self) {
        self.drag = None;
    }

    /// The wheel: zoom, keeping the block under the pointer where it is
    /// (while following the player, on the player).
    pub fn scrolled(&mut self, at: [f32; 2], dy: f64) {
        if dy == 0.0 || self.dialog.is_some() {
            return;
        }
        let under = self.view().world(at);
        self.scale = step_zoom(&MAP_ZOOMS, self.scale, dy > 0.0);
        if self.follow {
            return;
        }
        let m = self.view().mid();
        let s = f64::from(self.scale);
        self.centre = [under[0] - f64::from(at[0] - m[0]) / s, under[1] - f64::from(at[1] - m[1]) / s];
    }

    /// A character typed: into the dialog's name. True when taken.
    pub fn typed(&mut self, c: char) -> bool {
        let Some(d) = self.dialog.as_mut() else { return false };
        if !c.is_control() && d.name.chars().count() < 32 {
            d.name.push(c);
        }
        true
    }

    /// A key on the screen: Backspace and Enter for the dialog. True when taken.
    pub fn key(&mut self, key: &str) -> (bool, Action) {
        let Some(d) = self.dialog.as_mut() else { return (false, Action::Nothing) };
        match key {
            "key.keyboard.backspace" => {
                d.name.pop();
                (true, Action::Nothing)
            }
            "key.keyboard.enter" | "key.keyboard.keypad.enter" => (true, self.save()),
            _ => (true, Action::Nothing),
        }
    }

    fn save(&mut self) -> Action {
        let Some(d) = self.dialog.take() else { return Action::Nothing };
        let name = d.name.trim();
        let name = if name.is_empty() { "Waypoint" } else { name };
        Action::Save(name.to_owned(), d.at, COLOURS[d.colour % COLOURS.len()])
    }

    /// The whole screen: the map, the player and others, waypoints, the top
    /// bar, the list, the menu or dialog, and the block under the pointer.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        map: &Map,
        me: [f64; 3],
        yaw: f32,
        dots: &[Dot],
        list: &[&Waypoint],
        title: &str,
        mouse: [f32; 2],
        measure: &dyn Fn(&str) -> f32,
    ) -> Vec<Draw> {
        let v = self.view();
        let mut out = vec![Draw::Fill(v.rect, 0xFF10_1010)];
        let cell = if self.scale >= 2.0 { self.scale } else { 2.0 };
        cells(map, &v, cell, &mut out);
        // Chunk lines when close enough to tell them apart.
        if self.scale >= 3.0 {
            let w0 = v.world([0.0, 0.0]);
            let mut x = (w0[0] / 16.0).floor() * 16.0;
            while v.screen([x, 0.0])[0] < self.size[0] {
                let sx = v.screen([x, 0.0])[0];
                out.push(Draw::Fill([sx, 0.0, 1.0, self.size[1]], 0x2000_0000));
                x += 16.0;
            }
            let mut z = (w0[1] / 16.0).floor() * 16.0;
            while v.screen([0.0, z])[1] < self.size[1] {
                let sy = v.screen([0.0, z])[1];
                out.push(Draw::Fill([0.0, sy, self.size[0], 1.0], 0x2000_0000));
                z += 16.0;
            }
        }
        for d in dots.iter().filter(|d| d.kind == Kind::Player) {
            let p = v.screen([d.at[0], d.at[2]]);
            if v.inside(p, 0.0) {
                out.push(Draw::Fill([p[0] - 2.5, p[1] - 2.5, 5.0, 5.0], 0xFF00_0000));
                out.push(Draw::Fill([p[0] - 2.0, p[1] - 2.0, 4.0, 4.0], 0xFFFF_FFFF));
                if let Some(n) = &d.name {
                    centred(n, [p[0], p[1] - 8.0], 0xFFFFFF, measure, &mut out);
                }
            }
        }
        for w in list {
            let p = v.screen(w.xz());
            if v.inside(p, -6.0) {
                marker(w, p, measure, &mut out);
                if self.scale >= 0.5 {
                    centred(&w.name, [p[0], p[1] + 11.0], 0xFFFFFF, measure, &mut out);
                }
            }
        }
        let y = f64::from(yaw).to_radians();
        arrow(v.screen([me[0], me[2]]), [(-y.sin()) as f32, (y.cos()) as f32], &mut out);

        // The top bar.
        out.push(Draw::Fill([0.0, 0.0, self.size[0], BAR], 0xC000_0000));
        out.push(Draw::Text(title.to_owned(), [5.0, 5.0], 0xFFFFFF, true));
        for (r, text, on) in [(self.follow_button(), "Bana ortala", self.follow), (self.list_button(), "Waypointler", self.list)] {
            button(r, text, on, hit(r, mouse), measure, &mut out);
        }
        if self.list {
            let l = self.list_rect();
            out.push(Draw::Fill(l, 0xB000_0000));
            out.push(Draw::Text(format!("Waypointler ({})", list.len()), [l[0] + 4.0, l[1] + 3.0], 0xFFFF55, true));
            for (i, w) in list.iter().enumerate().take(self.rows()) {
                let r = self.row(i);
                if hit(r, mouse) {
                    out.push(Draw::Fill(r, 0x40FF_FFFF));
                }
                out.push(Draw::Fill([r[0] + 1.0, r[1] + 2.0, 8.0, 8.0], 0xFF00_0000 | w.colour));
                let d = ((f64::from(w.at[0]) + 0.5 - me[0]).powi(2) + (f64::from(w.at[2]) + 0.5 - me[2]).powi(2)).sqrt();
                let text = format!("{} · {} m", w.name, d.round() as i64);
                out.push(Draw::Text(text, [r[0] + 12.0, r[1] + 2.0], 0xFFFFFF, false));
                out.push(Draw::Text("×".into(), [r[0] + r[2] - 8.0, r[1] + 2.0], if hit([r[0] + r[2] - 10.0, r[1], 10.0, ROW], mouse) { 0xFF5555 } else { 0xAAAAAA }, false));
            }
            if list.len() > self.rows() {
                out.push(Draw::Text(format!("+{} daha", list.len() - self.rows()), [l[0] + 4.0, l[1] + l[3] - 11.0], 0xAAAAAA, false));
            }
        }
        // The bottom line: the block under the pointer, the zoom, the hints.
        let under = v.world(mouse);
        let (bx, bz) = (under[0].floor() as i32, under[1].floor() as i32);
        let at = match map.column(bx, bz) {
            Some(c) => format!("X {bx}  Y {}  Z {bz}", c.height),
            None => format!("X {bx}  Z {bz}"),
        };
        let zoom = if self.scale >= 1.0 { format!("{}:1", self.scale) } else { format!("1:{}", (1.0 / self.scale).round()) };
        let hints = "Sürükle: kaydır · Tekerlek: yakınlaştır · Sağ tık: menü";
        out.push(Draw::Fill([0.0, self.size[1] - 14.0, self.size[0], 14.0], 0xC000_0000));
        out.push(Draw::Text(format!("{at}   {zoom}"), [5.0, self.size[1] - 11.0], 0xFFFFFF, true));
        out.push(Draw::Text(hints.into(), [self.size[0] - measure(hints) - 5.0, self.size[1] - 11.0], 0xAAAAAA, true));

        if let Some((m, _)) = self.menu {
            for (i, text) in MENU_ITEMS.iter().enumerate() {
                let r = self.menu_item(m, i);
                out.push(Draw::Fill(r, if hit(r, mouse) { 0xF040_4040 } else { 0xF020_2020 }));
                out.push(Draw::Text((*text).to_owned(), [r[0] + 4.0, r[1] + 3.0], 0xFFFFFF, false));
            }
        }
        if let Some(d) = &self.dialog {
            let r = self.dialog_rect();
            let (field, swatch, save, cancel) = self.dialog_parts();
            out.push(Draw::Fill([0.0, 0.0, self.size[0], self.size[1]], 0x6000_0000));
            out.push(Draw::Fill([r[0] - 1.0, r[1] - 1.0, r[2] + 2.0, r[3] + 2.0], 0xFF80_8080));
            out.push(Draw::Fill(r, 0xFF20_2020));
            out.push(Draw::Text("Yeni waypoint".into(), [r[0] + 8.0, r[1] + 7.0], 0xFFFF55, true));
            out.push(Draw::Fill(field, 0xFF00_0000));
            out.push(Draw::Text(format!("{}_", d.name), [field[0] + 3.0, field[1] + 3.0], 0xFFFFFF, false));
            out.push(Draw::Fill([swatch[0] - 1.0, swatch[1] - 1.0, swatch[2] + 2.0, swatch[3] + 2.0], 0xFFFF_FFFF));
            out.push(Draw::Fill(swatch, 0xFF00_0000 | COLOURS[d.colour % COLOURS.len()]));
            out.push(Draw::Text(format!("{}, {}, {}", d.at[0], d.at[1], d.at[2]), [swatch[0] + 20.0, swatch[1] + 3.0], 0xDDDDDD, false));
            button(save, "Kaydet", false, hit(save, mouse), measure, &mut out);
            button(cancel, "İptal", false, hit(cancel, mouse), measure, &mut out);
        }
        out
    }
}

fn button(r: [f32; 4], text: &str, on: bool, hover: bool, measure: &dyn Fn(&str) -> f32, out: &mut Vec<Draw>) {
    out.push(Draw::Fill(r, if hover { 0xFF50_5050 } else if on { 0xFF30_4A30 } else { 0xFF30_3030 }));
    out.push(Draw::Text(text.to_owned(), [r[0] + (r[2] - measure(text)) / 2.0, r[1] + (r[3] - FONT) / 2.0 + 1.0], 0xFFFFFF, true));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure(s: &str) -> f32 {
        s.chars().count() as f32 * 6.0
    }

    fn flat(rgb: u32, height: i16) -> Cols {
        Box::new([Column { rgb, height, water: 0 }; 256])
    }

    #[test]
    fn columns_live_in_their_chunk() {
        let mut m = Map::default();
        let mut cols = empty();
        cols[index(-1, -1)] = Column { rgb: 0x123456, height: 70, water: 0 };
        assert!(m.put(-1, -1, cols.clone()));
        assert!(!m.put(-1, -1, cols), "the same chunk again is not a change");
        assert_eq!(m.column(-1, -1).map(|c| c.rgb), Some(0x123456));
        assert_eq!(m.column(-2, -1), None, "unscanned columns are unknown");
        assert!(m.dirty.contains(&(-1, -1)));
    }

    #[test]
    fn regions_round_trip() {
        let mut m = Map::default();
        m.put(0, 0, flat(0xABCDEF, -64));
        m.put(31, 31, flat(0x010203, 300));
        m.put(32, 0, flat(0x111111, 5));
        let bytes = m.region(0, 0);
        let list = decode(&bytes).expect("decodes");
        assert_eq!(list.len(), 2, "region 0,0 holds chunks 0..31 only");
        let mut back = Map::default();
        assert!(back.load(&bytes));
        assert_eq!(back.column(31 * 16 + 5, 31 * 16 + 5).map(|c| (c.rgb, c.height)), Some((0x010203, 300)));
        assert!(decode(b"PSL1\x01\x00\x00\x00").is_none(), "truncated files are refused");
    }

    #[test]
    fn looks() {
        assert_eq!(look("minecraft:air"), Look::Clear);
        assert_eq!(look("minecraft:white_stained_glass"), Look::Clear);
        assert_eq!(look("minecraft:water[level=0]"), Look::Water);
        assert!(matches!(look("minecraft:grass_block[snowy=false]"), Look::Block(0x6A9F3C, true)));
        assert!(matches!(look("minecraft:oak_leaves[distance=1]"), Look::Block(_, false)));
        assert!(matches!(look("minecraft:light_gray_wool"), Look::Block(0x8E8E86, true)), "light_gray, not gray");
        assert!(matches!(look("minecraft:dark_oak_planks"), Look::Block(0x42291A, true)), "dark_oak, not oak");
        assert!(matches!(look("minecraft:spruce_log[axis=y]"), Look::Block(_, false)));
        assert!(matches!(look("minecraft:deepslate_diamond_ore"), Look::Block(0x505053, true)));
        assert!(matches!(look("minecraft:something_new"), Look::Block(0x8A8A8A, true)));
    }

    #[test]
    fn scanning_finds_the_top_through_water() {
        // 70: water, 69: water, 68: sand.
        let mut at = |y: i32| match y {
            70 | 69 => Cell::Water,
            68 => Cell::Block(0xDBCFA3),
            _ => Cell::Air,
        };
        let c = scan(100, 0, &mut at);
        assert_eq!((c.rgb, c.height, c.water), (0xDBCFA3, 70, 2));
        let dry = scan(100, 0, &mut |y| if y == 64 { Cell::Block(1) } else { Cell::Air });
        assert_eq!((dry.height, dry.water), (64, 0));
        assert_eq!(scan(100, 0, &mut |_| Cell::Unloaded), Column::NONE);
        let deep = scan(100, 0, &mut |y| if y <= 80 { Cell::Water } else { Cell::Air });
        assert_eq!((deep.height, deep.water), (80, DEEP), "deep water stops looking");
    }

    #[test]
    fn hinted_scans_read_little_and_find_new_blocks() {
        // Ground at 64; a tower since built up to 90.
        let mut reads = 0;
        let ground = |y: i32| if y <= 64 { Cell::Block(1) } else { Cell::Air };
        let c = scan_hinted(319, -64, Some(64), &mut |y| {
            reads += 1;
            ground(y)
        });
        assert_eq!(c.height, 64);
        assert!(reads <= 10, "{reads} reads");
        let tower = scan_hinted(319, -64, Some(64), &mut |y| if y <= 90 { Cell::Block(2) } else { Cell::Air });
        assert_eq!((tower.rgb, tower.height), (2, 90));
    }

    #[test]
    fn colours_mix() {
        assert_eq!(mix(0x000000, 0xFFFFFF, 0.5), 0x808080);
        assert_eq!(scale(0x808080, 2.0), 0xFFFFFF);
        assert_eq!(base(Column { rgb: 0xFF0000, height: 1, water: 0 }), 0xFF0000);
        assert_ne!(base(Column { rgb: 0xFF0000, height: 1, water: 3 }), 0xFF0000);
    }

    #[test]
    fn views_go_both_ways() {
        for rotate in [false, true] {
            for yaw in [0.0_f32, 37.0, 90.0, -135.0, 180.0] {
                let v = View { centre: [100.5, -20.25], yaw, rotate, scale: 2.0, rect: [10.0, 10.0, 100.0, 100.0], round: false };
                let w = [130.0, -5.0];
                let back = v.world(v.screen(w));
                assert!((back[0] - w[0]).abs() < 1e-3 && (back[1] - w[1]).abs() < 1e-3, "{rotate} {yaw}");
                assert_eq!(v.screen(v.centre), v.mid());
            }
        }
        // Not turned, north is up; turned, the way you face is up.
        let v = View { centre: [0.0, 0.0], yaw: 0.0, rotate: false, scale: 1.0, rect: [0.0, 0.0, 100.0, 100.0], round: false };
        assert!(v.screen([0.0, -10.0])[1] < 50.0);
        let t = View { yaw: 0.0, rotate: true, ..v };
        assert!(t.screen([0.0, 10.0])[1] < 50.0, "yaw 0 faces south (+z), drawn up");
        let w = View { yaw: 90.0, rotate: true, ..v };
        assert!(w.screen([-10.0, 0.0])[1] < 50.0, "yaw 90 faces west (-x), drawn up");
    }

    #[test]
    fn markers_stay_on_the_edge() {
        let v = View { centre: [0.0, 0.0], yaw: 0.0, rotate: false, scale: 1.0, rect: [0.0, 0.0, 100.0, 100.0], round: true };
        let p = v.clamp([500.0, 50.0], 5.0);
        assert!((p[0] - 95.0).abs() < 1e-3 && (p[1] - 50.0).abs() < 1e-3);
        let sq = View { round: false, ..v };
        assert_eq!(sq.clamp([500.0, -500.0], 5.0), [95.0, 5.0]);
    }

    #[test]
    fn cells_merge_runs() {
        let mut m = Map::default();
        for cx in -2..2 {
            for cz in -2..2 {
                m.put(cx, cz, flat(0x336699, 64));
            }
        }
        let v = View { centre: [0.0, 0.0], yaw: 0.0, rotate: false, scale: 1.0, rect: [0.0, 0.0, 40.0, 40.0], round: false };
        let mut out = Vec::new();
        cells(&m, &v, 4.0, &mut out);
        assert_eq!(out.len(), 10, "one fill a row on flat ground");
        let Draw::Fill(r, _) = &out[0] else { panic!("a fill") };
        assert!(r[0] == 0.0 && r[1] == 0.0 && (r[2] - 40.0).abs() < 1e-3 && r[3] == 4.0);
    }

    #[test]
    fn waypoints_round_trip() {
        let list = vec![
            Waypoint { name: "Ev\tbarınak".into(), at: [10, 64, -300], colour: 0xFF5555, dimension: "minecraft:overworld".into(), death: false },
            Waypoint { name: "Ölüm".into(), at: [-1, -50, 2], colour: 0x555555, dimension: "minecraft:the_nether".into(), death: true },
        ];
        let back = read_waypoints(&write_waypoints(&list));
        assert_eq!(back[0].name, "Ev barınak", "tabs cannot break the file");
        assert_eq!(back[1], list[1]);
        assert_eq!(list[1].initial(), "Ö");
        assert!(read_waypoints("bozuk satır\n").is_empty());
    }

    #[test]
    fn folders_are_safe() {
        assert_eq!(folder("Play.Example.NET:25565"), "play.example.net_25565");
        assert_eq!(folder("../.."), ".._..");
        assert_eq!(folder(".."), "_");
        assert_eq!(folder(""), "_");
    }

    #[test]
    fn projection_puts_ahead_in_the_middle() {
        let size = [400.0, 200.0];
        let p = project([0.0, 64.0, 0.0], 0.0, 0.0, 70.0, size, [0.0, 64.0, 10.0]).expect("ahead");
        assert!((p[0] - 200.0).abs() < 1e-3 && (p[1] - 100.0).abs() < 1e-3);
        assert!(project([0.0, 64.0, 0.0], 0.0, 0.0, 70.0, size, [0.0, 64.0, -10.0]).is_none(), "behind");
        let up = project([0.0, 64.0, 0.0], 0.0, 0.0, 70.0, size, [0.0, 70.0, 10.0]).unwrap();
        assert!(up[1] < 100.0, "higher is up the screen");
        let right = project([0.0, 64.0, 0.0], 0.0, 0.0, 70.0, size, [-3.0, 64.0, 10.0]).unwrap();
        assert!(right[0] > 200.0, "facing south, west (-x) is on the right");
    }

    #[test]
    fn settings_read_and_check() {
        let s = Settings::read(&|_| None);
        assert!(s.minimap && s.rotate && !s.round);
        assert_eq!((s.size, s.zoom, s.detail, s.corner, s.cave), (100.0, 1.0, 80, Corner::TopLeft, Cave::Auto));
        assert_eq!(checked("shape", "Daire").as_deref(), Ok("daire"));
        assert_eq!(checked("zoom", "1,5").as_deref(), Ok("1.5"));
        assert!(checked("zoom", "7").is_err());
        assert!(checked("size", "1000").is_err());
        let bad = Settings::read(&|k| (k == "size").then(|| "çok".to_owned()));
        assert_eq!(bad.size, 100.0, "a bad stored value falls back");
        assert_eq!(step_zoom(&ZOOMS, 1.0, true), 1.5);
        assert_eq!(step_zoom(&ZOOMS, 0.5, false), 0.5);
    }

    #[test]
    fn minimap_draws() {
        let s = Settings::read(&|_| None);
        let m = Map::default();
        let w = Waypoint { name: "ev".into(), at: [1000, 64, 0], colour: 0x55FF55, dimension: "minecraft:overworld".into(), death: false };
        let wps = [&w];
        let mini = Mini { settings: &s, map: &m, me: [0.0, 64.0, 0.0], yaw: 0.0, dots: &[], waypoints: &wps, screen: [480.0, 270.0], cave: false };
        let out = minimap(&mini, &measure);
        assert!(out.iter().any(|d| matches!(d, Draw::Text(t, _, _, _) if t == "E")), "the far waypoint's letter, on the edge");
        assert!(out.iter().any(|d| matches!(d, Draw::Text(t, _, _, _) if t == "0, 64, 0")));
    }

    #[test]
    fn the_world_map_screen() {
        let mut s = Screen { size: [480.0, 270.0], ..Screen::default() };
        let none = |_: i32, _: i32| Some(70);
        s.opened([5.5, 64.0, 5.5], false, 0);
        // Drag the map 20 px left: the centre moves 20 blocks east at 1:1.
        assert_eq!(s.press([200.0, 100.0], 0, &[], &none), Action::Nothing);
        s.dragged([180.0, 100.0]);
        s.released();
        assert!((s.centre[0] - 25.5).abs() < 1e-6 && !s.follow);
        // Right click, then "Işınlan".
        s.press([240.0, 135.0], 1, &[], &none);
        let item = s.menu_item([240.0, 135.0], 1);
        let a = s.press([item[0] + 2.0, item[1] + 2.0], 0, &[], &none);
        assert_eq!(a, Action::Command("/tp @s 25 71 5".into()));
        // A new waypoint: typed, then Enter.
        s.opened([0.0, 64.0, 0.0], true, 2);
        assert!(s.typed('!'));
        assert_eq!(s.key("key.keyboard.enter").1, Action::Save("Waypoint 3!".into(), [0, 64, 0], COLOURS[2]));
        assert!(!s.typed('x'), "no dialog, no typing");
        // Zooming keeps the block under the pointer (once not following).
        s.follow = false;
        let before = s.view().world([100.0, 50.0]);
        s.scrolled([100.0, 50.0], 1.0);
        let after = s.view().world([100.0, 50.0]);
        assert!((before[0] - after[0]).abs() < 1e-6 && (before[1] - after[1]).abs() < 1e-6);
        assert!(s.scale > 1.0);
        let out = s.draw(&Map::default(), [0.0, 64.0, 0.0], 0.0, &[], &[], "Pusula", [10.0, 10.0], &measure);
        assert!(out.iter().any(|d| matches!(d, Draw::Text(t, _, _, _) if t == "Waypointler (0)")));
    }
}
