// Eksen — 16.10.1 (mihehk): what every editor tool shares. Written from
// scratch after the behaviour Axiom's documentation describes
// (.build-tools/axiom-docs/axiom-docs.md: "Tools", "Brushes", "Target
// Blocks"); no code, names or images of Axiom are used.
//
// A tool is plain Rust: it reads the world through `Blocks`, takes the
// stroke the player made (`Input`) and answers with an `Outcome`, which the
// editor turns into commands. Tools live in their own modules
// (`eksen_tools_*`) and are listed by `all()`.
use std::collections::{BTreeMap, BTreeSet};

pub use super::eksen_magic::{Block, Blocks, Pos};

/// A brush's shape (the default is a sphere).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Sphere,
    Cube,
    Octahedron,
}

impl Shape {
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "kure" | "küre" | "sphere" => Shape::Sphere,
            "kup" | "küp" | "cube" => Shape::Cube,
            "sekizyuzlu" | "sekizyüzlü" | "octahedron" => Shape::Octahedron,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Shape::Sphere => "küre",
            Shape::Cube => "küp",
            Shape::Octahedron => "sekizyüzlü",
        }
    }

    /// Whether offset `d` from the centre is inside a brush of `radius`.
    pub fn contains(self, d: Pos, radius: i32) -> bool {
        let [x, y, z] = d.map(i64::from);
        let r = i64::from(radius);
        match self {
            // Half a block past the radius, so radius 0 is one block and the
            // ball has no single-block spikes on its axes.
            Shape::Sphere => 4 * (x * x + y * y + z * z) <= (2 * r + 1) * (2 * r + 1),
            Shape::Cube => x.abs() <= r && y.abs() <= r && z.abs() <= r,
            Shape::Octahedron => x.abs() + y.abs() + z.abs() <= r,
        }
    }
}

/// The blocks of one brush of `shape` and `radius` at `centre`.
pub fn brush(centre: Pos, radius: i32, shape: Shape) -> Vec<Pos> {
    let r = radius.max(0);
    let mut out = Vec::new();
    for dx in -r..=r {
        for dy in -r..=r {
            for dz in -r..=r {
                if shape.contains([dx, dy, dz], r) {
                    out.push([centre[0] + dx, centre[1] + dy, centre[2] + dz]);
                }
            }
        }
    }
    out
}

/// Every block a stroke's brushes cover: the target blocks.
pub fn stroke(points: &[Pos], radius: i32, shape: Shape) -> BTreeSet<Pos> {
    points.iter().flat_map(|p| brush(*p, radius, shape)).collect()
}

/// What the player did with a tool: the blocks hit while its key was held,
/// in order, and what else the tool may want.
#[derive(Clone, Debug, Default)]
pub struct Input {
    /// Blocks hit (the looked-at block) while the key was held, in order,
    /// without repeats; the first is where the stroke began.
    pub stroke: Vec<Pos>,
    /// The face of the first hit block the player looked at, a unit step
    /// out of it (`[0, 1, 0]`: the top).
    pub face: Pos,
    /// The player's eyes and view direction (unit vector) when released.
    pub eye: [f64; 3],
    pub look: [f64; 3],
    /// The active block: what tools place, `minecraft:stone` or with a
    /// state, `minecraft:oak_stairs[facing=east]`.
    pub active: String,
    /// The current selection, empty if none (a box selection's blocks are
    /// listed out when it is small enough).
    pub selection: BTreeSet<Pos>,
    /// The second action of the tool (erase, lower, subtract …), as Axiom
    /// gives it on the other mouse button.
    pub secondary: bool,
    /// Free text a tool asks for (the Text tool's text).
    pub text: String,
}

/// What a tool did.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Blocks to set: position to block string (`minecraft:air` to clear).
    Edit(BTreeMap<Pos, String>),
    /// A new selection (joined to the current one by the editor's
    /// boolean operation).
    Select(BTreeSet<Pos>),
    /// Biomes to set over boxes: (min, max, biome id).
    Biomes(Vec<(Pos, Pos, String)>),
    /// Chat lines only (a measurement, an error).
    Message(Vec<String>),
}

/// An editor tool.
pub trait Tool: Send {
    /// The word that picks it: `/eksen arac <id>` (ascii, Turkish).
    fn id(&self) -> &'static str;
    /// Its name as shown.
    fn name(&self) -> &'static str;
    /// Sets option `option` to `value` (`/eksen ayar <option> <value>`);
    /// a toggle flips with no value.
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String>;
    /// The options and their values, a line each.
    fn describe(&self) -> Vec<String>;
    /// Runs the tool on a stroke.
    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome;
}

/// A block as a command writes it: `minecraft:oak_stairs[facing=east]`.
pub fn block_string(b: &Block) -> String {
    if b.state.is_empty() {
        b.id.clone()
    } else {
        format!("{}[{}]", b.id, b.state)
    }
}

/// The air block string.
pub const AIR: &str = "minecraft:air";

/// Parses a number option within `lo..=hi`.
pub fn number<T: std::str::FromStr + PartialOrd + std::fmt::Display + Copy>(
    value: Option<&str>,
    lo: T,
    hi: T,
) -> Result<T, String> {
    let v: T = value.and_then(|w| w.parse().ok()).ok_or_else(|| format!("Bir sayı gerekli ({lo}–{hi})."))?;
    if v < lo || v > hi {
        return Err(format!("Değer {lo} ile {hi} arasında olmalı."));
    }
    Ok(v)
}

/// Flips a toggle, or sets it from `ac`/`kapat` (`on`/`off`).
pub fn toggle(flag: &mut bool, value: Option<&str>) -> Result<(), String> {
    *flag = match value {
        None => !*flag,
        Some("ac" | "aç" | "on" | "true") => true,
        Some("kapat" | "off" | "false") => false,
        Some(w) => return Err(format!("aç veya kapat bekleniyordu: {w}")),
    };
    Ok(())
}

/// "açık" / "kapalı".
pub fn on_off(b: bool) -> &'static str {
    if b {
        "açık"
    } else {
        "kapalı"
    }
}

/// A small deterministic random source (xorshift64*), so a tool gives the
/// same result for the same seed.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15 | 1)
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// In `0.0..1.0`.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Smooth 3D value noise in `-1.0..=1.0` at `p` (in blocks) scaled by
/// `scale` blocks per feature, for `seed`.
pub fn noise3(p: [f64; 3], scale: f64, seed: u64) -> f64 {
    let s = scale.max(0.001);
    let q = p.map(|v| v / s);
    let i = q.map(|v| v.floor() as i64);
    let f = [0, 1, 2].map(|k| q[k] - i[k] as f64);
    let fade = f.map(|t| t * t * (3.0 - 2.0 * t));
    let corner = |dx: i64, dy: i64, dz: i64| {
        let mut h = seed ^ 0xD6E8_FEB8_6659_FD93;
        for v in [i[0] + dx, i[1] + dy, i[2] + dz] {
            h = (h ^ v as u64).wrapping_mul(0x1000_0000_01B3);
            h ^= h >> 29;
        }
        (h >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    };
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let x00 = lerp(corner(0, 0, 0), corner(1, 0, 0), fade[0]);
    let x10 = lerp(corner(0, 1, 0), corner(1, 1, 0), fade[0]);
    let x01 = lerp(corner(0, 0, 1), corner(1, 0, 1), fade[0]);
    let x11 = lerp(corner(0, 1, 1), corner(1, 1, 1), fade[0]);
    lerp(lerp(x00, x10, fade[1]), lerp(x01, x11, fade[1]), fade[2])
}

/// Every tool, in the order Axiom's documentation lists them. Each module
/// adds its own.
pub fn all() -> Vec<Box<dyn Tool>> {
    let mut out: Vec<Box<dyn Tool>> = Vec::new();
    out.extend(super::eksen_tools_select::tools());
    out.extend(super::eksen_tools_paint::tools());
    out.extend(super::eksen_tools_draw::tools());
    out.extend(super::eksen_tools_terrain::tools());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brush_shapes() {
        assert_eq!(brush([0, 0, 0], 0, Shape::Sphere).len(), 1);
        assert_eq!(brush([0, 0, 0], 1, Shape::Cube).len(), 27);
        assert_eq!(brush([0, 0, 0], 1, Shape::Octahedron).len(), 7);
        // A radius 1 ball: the 3 cube less its 8 corners.
        assert_eq!(brush([0, 0, 0], 1, Shape::Sphere).len(), 19);
        assert_eq!(stroke(&[[0, 0, 0], [1, 0, 0]], 0, Shape::Cube).len(), 2);
    }

    #[test]
    fn noise_is_smooth_and_bounded() {
        let a = noise3([0.0, 0.0, 0.0], 8.0, 1);
        let b = noise3([0.1, 0.0, 0.0], 8.0, 1);
        assert!((a - b).abs() < 0.1);
        for i in 0..200 {
            let v = noise3([i as f64 * 1.7, i as f64 * 0.3, -(i as f64)], 5.0, 7);
            assert!((-1.0..=1.0).contains(&v));
        }
        assert_eq!(noise3([3.0, 4.0, 5.0], 4.0, 9), noise3([3.0, 4.0, 5.0], 4.0, 9));
    }

    #[test]
    fn options_parse() {
        assert_eq!(number::<i32>(Some("5"), 1, 10), Ok(5));
        assert!(number::<i32>(Some("50"), 1, 10).is_err());
        let mut f = false;
        toggle(&mut f, None).unwrap();
        assert!(f);
        toggle(&mut f, Some("kapat")).unwrap();
        assert!(!f);
    }
}
