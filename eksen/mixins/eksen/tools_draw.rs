// Eksen — 16.10.1 (mihehk): the drawing and modelling tools (Freehand Draw,
// Sculpt Draw, Rock, Weld, Melt, Stamp, Text, Shape Tool, Path). Written
// from scratch after the behaviour Axiom's documentation describes
// (.build-tools/axiom-docs/axiom-docs.md: "Drawing Tools" and "Modelling");
// no code, names or images of Axiom are used.
//
// Every tool is a plain `Tool` over the `Blocks` trait: it reads the world,
// takes the stroke the player made and answers with the blocks to set. Work
// is capped at `MAX_BLOCKS` blocks per use. Where the documentation gives no
// number, the default is marked `// Inferred:`.
use super::eksen_core::*;
use std::collections::{BTreeMap, BTreeSet};

/// The most blocks one use of a tool may change.
pub const MAX_BLOCKS: usize = 1_000_000;
/// The most cells of a scratch grid (blurs, shape masks).
const GRID_CAP: usize = 8_000_000;
/// The six steps to a neighbouring block.
const DIRS: [Pos; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: Pos, b: Pos) -> Pos {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn mul(a: Pos, k: i32) -> Pos {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn round_pos(p: [f64; 3]) -> Pos {
    [p[0].round() as i32, p[1].round() as i32, p[2].round() as i32]
}

fn dist(a: Pos, b: Pos) -> f64 {
    let d = sub(a, b).map(f64::from);
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

fn dist3(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// Anything but air (water and flowers count).
fn filled(world: &dyn Blocks, p: Pos) -> bool {
    world.block(p).map_or(false, |b| !b.air())
}

/// A block that cannot be walked through.
fn solid_at(world: &dyn Blocks, p: Pos) -> bool {
    world.block(p).map_or(false, |b| !b.air() && b.solid)
}

/// Air, or something a build may replace without a thought (water, flowers).
fn replaceable(world: &dyn Blocks, p: Pos) -> bool {
    world.block(p).map_or(false, |b| b.air() || !b.solid)
}

fn msg(text: &str) -> Outcome {
    Outcome::Message(vec![text.to_string()])
}

fn too_many() -> Outcome {
    Outcome::Message(vec![format!("Çok fazla blok: sınır {MAX_BLOCKS}.")])
}

/// The edits, or a message when there are none.
fn done(out: BTreeMap<Pos, String>) -> Outcome {
    if out.is_empty() {
        msg("Değişen blok yok.")
    } else {
        Outcome::Edit(out)
    }
}

fn unknown(option: &str) -> Result<(), String> {
    Err(format!("Bilinmeyen ayar: {option}"))
}

/// A seed from a position: the same stroke start gives the same rock.
fn seed_of(p: Pos) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for v in p {
        h = (h ^ u64::from(v as u32)).wrapping_mul(0x0000_0100_0000_01b3);
        h ^= h >> 32;
    }
    h
}

/// Whether `points` brushes of `radius` would be too much work.
fn brush_too_big(points: usize, radius: i32) -> bool {
    let side = (2 * radius.max(0) + 1) as usize;
    points.saturating_mul(side * side * side) > 64_000_000
}

/// Keeps a point only when it is `min_dist` from the last one kept.
fn thin(points: &[Pos], min_dist: i32) -> Vec<Pos> {
    let md2 = i64::from(min_dist.max(1)).pow(2);
    let mut out: Vec<Pos> = Vec::new();
    for p in points {
        let far = out.last().map_or(true, |q| {
            let d: i64 = (0..3)
                .map(|k| {
                    let t = i64::from(p[k] - q[k]);
                    t * t
                })
                .sum();
            d >= md2
        });
        if far {
            out.push(*p);
        }
    }
    out
}

/// The axis and sign of a face step (`[0, 1, 0]`: axis 1, sign 1); up when
/// the face is unknown.
fn normal_axis(face: Pos) -> (usize, i32) {
    for a in 0..3 {
        if face[a] != 0 {
            return (a, face[a].signum());
        }
    }
    (1, 1)
}

/// The two axes other than `a`.
fn others(a: usize) -> (usize, usize) {
    match a {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    }
}

/// A scratch grid of numbers over a box of blocks, for blurs and masks.
struct Grid {
    min: Pos,
    dim: [usize; 3],
    data: Vec<f32>,
}

impl Grid {
    fn new(min: Pos, dim: [usize; 3]) -> Option<Grid> {
        let n = dim[0].checked_mul(dim[1])?.checked_mul(dim[2])?;
        if n == 0 || n > GRID_CAP {
            return None;
        }
        Some(Grid { min, dim, data: vec![0.0; n] })
    }

    fn index(&self, c: [usize; 3]) -> usize {
        (c[0] * self.dim[1] + c[1]) * self.dim[2] + c[2]
    }

    fn pos(&self, c: [usize; 3]) -> Pos {
        [self.min[0] + c[0] as i32, self.min[1] + c[1] as i32, self.min[2] + c[2] as i32]
    }

    fn cell_of(&self, p: Pos) -> Option<[usize; 3]> {
        let mut c = [0usize; 3];
        for k in 0..3 {
            let d = p[k] - self.min[k];
            if d < 0 || d as usize >= self.dim[k] {
                return None;
            }
            c[k] = d as usize;
        }
        Some(c)
    }

    /// The value at a block, 0 outside the grid.
    fn get(&self, p: Pos) -> f32 {
        self.cell_of(p).map_or(0.0, |c| self.data[self.index(c)])
    }

    /// A Gaussian blur of standard deviation `sd` blocks (separable; the
    /// grid's edge repeats outwards).
    fn blur(&mut self, sd: f64) {
        if sd <= 0.0 {
            return;
        }
        let k = ((2.0 * sd).ceil() as i32).clamp(1, 12);
        let mut w: Vec<f32> = (-k..=k).map(|i| (-((i * i) as f64) / (2.0 * sd * sd)).exp() as f32).collect();
        let total: f32 = w.iter().sum();
        for v in w.iter_mut() {
            *v /= total;
        }
        for axis in 0..3 {
            let src = self.data.clone();
            let mut dst = vec![0.0f32; src.len()];
            let last = self.dim[axis] as i64 - 1;
            for x in 0..self.dim[0] {
                for y in 0..self.dim[1] {
                    for z in 0..self.dim[2] {
                        let c = [x, y, z];
                        let mut acc = 0.0f32;
                        for (wi, o) in (-k..=k).enumerate() {
                            let mut cc = c;
                            cc[axis] = (c[axis] as i64 + i64::from(o)).clamp(0, last) as usize;
                            acc += w[wi] * src[self.index(cc)];
                        }
                        dst[self.index(c)] = acc;
                    }
                }
            }
            self.data = dst;
        }
    }
}

/// The radius that a blur of `sd` reaches (the grid padding).
fn blur_reach(sd: f64) -> i32 {
    if sd > 0.0 {
        ((2.0 * sd).ceil() as i32).clamp(1, 12)
    } else {
        0
    }
}

/// A direction: up, down or one of the four compass sides. Stamp uses it as
/// the way the stamp's top points, Text as the side the text faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    North,
    South,
    East,
    West,
}

impl Dir {
    pub fn parse(word: &str) -> Option<Dir> {
        Some(match word {
            "yukari" | "yukarı" | "up" => Dir::Up,
            "asagi" | "aşağı" | "down" => Dir::Down,
            "kuzey" | "north" => Dir::North,
            "guney" | "güney" | "south" => Dir::South,
            "dogu" | "doğu" | "east" => Dir::East,
            "bati" | "batı" | "west" => Dir::West,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Dir::Up => "yukarı",
            Dir::Down => "aşağı",
            Dir::North => "kuzey",
            Dir::South => "güney",
            Dir::East => "doğu",
            Dir::West => "batı",
        }
    }

    /// The unit step of the direction (north is -z, east +x).
    pub fn vector(self) -> Pos {
        match self {
            Dir::Up => [0, 1, 0],
            Dir::Down => [0, -1, 0],
            Dir::North => [0, 0, -1],
            Dir::South => [0, 0, 1],
            Dir::East => [1, 0, 0],
            Dir::West => [-1, 0, 0],
        }
    }

    /// Turns a local offset (up is +y) so that local up points this way;
    /// every case is a proper rotation.
    pub fn map(self, l: Pos) -> Pos {
        let [x, y, z] = l;
        match self {
            Dir::Up => [x, y, z],
            Dir::Down => [x, -y, -z],
            Dir::East => [y, -x, z],
            Dir::West => [-y, x, z],
            Dir::South => [x, -z, y],
            Dir::North => [x, z, -y],
        }
    }

    /// The reading direction (u, to the right) and the up direction (v) of
    /// text that faces this way, for a viewer looking at it.
    fn text_axes(self) -> (Pos, Pos) {
        match self {
            Dir::Up => ([1, 0, 0], [0, 0, -1]),
            Dir::Down => ([1, 0, 0], [0, 0, 1]),
            Dir::South => ([1, 0, 0], [0, 1, 0]),
            Dir::North => ([-1, 0, 0], [0, 1, 0]),
            Dir::East => ([0, 0, -1], [0, 1, 0]),
            Dir::West => ([0, 0, 1], [0, 1, 0]),
        }
    }
}

// ---------------------------------------------------------------------------
// Freehand Draw
// ---------------------------------------------------------------------------

/// Freehand Draw: the brush stroke, filled with the active block.
pub struct Freehand {
    pub radius: i32,
    pub shape: Shape,
}

impl Default for Freehand {
    fn default() -> Self {
        // Inferred: the documentation names the options but no radius.
        Freehand { radius: 3, shape: Shape::Sphere }
    }
}

impl Tool for Freehand {
    fn id(&self) -> &'static str {
        "ciz"
    }
    fn name(&self) -> &'static str {
        "Serbest Çizim"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        match option {
            "yaricap" | "yarıçap" | "radius" => self.radius = number(value, 0, 32)?,
            "sekil" | "şekil" | "shape" => {
                self.shape = value.and_then(Shape::parse).ok_or("küre, küp veya sekizyüzlü bekleniyordu.")?
            }
            _ => return unknown(option),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        vec![format!("yarıçap: {}", self.radius), format!("şekil: {}", self.shape.name())]
    }
    fn apply(&self, _world: &dyn Blocks, input: &Input) -> Outcome {
        if input.stroke.is_empty() {
            return msg("Vuruş yok.");
        }
        if brush_too_big(input.stroke.len(), self.radius) {
            return too_many();
        }
        let cells = stroke(&input.stroke, self.radius, self.shape);
        if cells.len() > MAX_BLOCKS {
            return too_many();
        }
        // The second action erases.
        let block = if input.secondary { AIR.to_string() } else { input.active.clone() };
        done(cells.into_iter().map(|p| (p, block.clone())).collect())
    }
}

// ---------------------------------------------------------------------------
// Sculpt Draw
// ---------------------------------------------------------------------------

/// Sculpt Draw: pulls the surface out along its normal (or presses it in).
pub struct Sculpt {
    pub radius: i32,
    /// How far the shape rises: 0 to 5.
    pub strength: f64,
    pub invert: bool,
    /// Only add mass along the Y axis.
    pub mask_y: bool,
    pub denoise: bool,
}

impl Default for Sculpt {
    fn default() -> Self {
        // Inferred: radius and strength have no documented default; denoise
        // is documented as "recommended on".
        Sculpt { radius: 5, strength: 1.0, invert: false, mask_y: false, denoise: true }
    }
}

impl Tool for Sculpt {
    fn id(&self) -> &'static str {
        "heykel"
    }
    fn name(&self) -> &'static str {
        "Heykel Çizimi"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        match option {
            "yaricap" | "yarıçap" | "radius" => self.radius = number(value, 1, 32)?,
            "guc" | "güç" | "strength" => self.strength = number(value, 0.0, 5.0)?,
            "ters" | "invert" => toggle(&mut self.invert, value)?,
            "maske_y" | "masky" => toggle(&mut self.mask_y, value)?,
            "gurultu_azalt" | "denoise" => toggle(&mut self.denoise, value)?,
            _ => return unknown(option),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        vec![
            format!("yarıçap: {}", self.radius),
            format!("güç: {}", self.strength),
            format!("ters: {}", on_off(self.invert)),
            format!("maske_y: {}", on_off(self.mask_y)),
            format!("gürültü_azalt: {}", on_off(self.denoise)),
        ]
    }
    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        if input.stroke.is_empty() {
            return msg("Vuruş yok.");
        }
        let invert = self.invert != input.secondary;
        let (a, s) = if self.mask_y { (1, 1) } else { normal_axis(input.face) };
        let (b, c) = others(a);
        let r = self.radius;
        let rf = f64::from(r);
        let mut changes: BTreeMap<Pos, String> = BTreeMap::new();
        for p in thin(&input.stroke, 1) {
            for i in -r..=r {
                for j in -r..=r {
                    let t = f64::from(i * i + j * j).sqrt();
                    if t > rf {
                        continue;
                    }
                    // Small radii give a parabola, large ones a ball.
                    let u = t / (rf + 0.5);
                    let profile = if r >= 4 { (1.0 - u * u).max(0.0).sqrt() } else { 1.0 - u * u };
                    let height = (self.strength * rf * 0.5 * profile + 0.5).floor() as i32;
                    if height < 1 {
                        continue;
                    }
                    let at = |h: i32| -> Pos {
                        let mut q = p;
                        q[b] += i;
                        q[c] += j;
                        q[a] += h * s;
                        q
                    };
                    // The topmost surface block of this column near the hit.
                    let mut surface = None;
                    for h in (-r..=r).rev() {
                        if filled(world, at(h)) && !filled(world, at(h + 1)) {
                            surface = Some(h);
                            break;
                        }
                    }
                    let Some(sh) = surface else { continue };
                    let Some(src) = world.block(at(sh)) else { continue };
                    let text = block_string(&src);
                    if invert {
                        for k in 0..height {
                            let q = at(sh - k);
                            if filled(world, q) {
                                changes.insert(q, AIR.to_string());
                            }
                        }
                    } else {
                        for k in 1..=height {
                            let q = at(sh + k);
                            if replaceable(world, q) {
                                changes.insert(q, text.clone());
                            }
                        }
                    }
                }
            }
            if changes.len() > MAX_BLOCKS {
                return too_many();
            }
        }
        if self.denoise {
            // A block needs two supporting neighbours (the surface or other
            // changed blocks), which drops lone spikes.
            let keep: BTreeSet<Pos> = changes
                .keys()
                .copied()
                .filter(|q| {
                    let n = DIRS
                        .iter()
                        .filter(|d| {
                            let nb = add(*q, **d);
                            changes.contains_key(&nb) || (filled(world, nb) != invert)
                        })
                        .count();
                    n >= 2
                })
                .collect();
            changes.retain(|q, _| keep.contains(q));
        }
        done(changes)
    }
}

// ---------------------------------------------------------------------------
// Rock
// ---------------------------------------------------------------------------

/// Rock: a noisy, smoothed blob of the active block set on the surface.
pub struct Rock {
    pub radius: i32,
    /// The scale of the noise in blocks.
    pub noise_radius: f64,
    /// How much the noise bends the ball: 0 is a smooth ball.
    pub noisiness: f64,
    /// `None`: a new seed for every stroke.
    pub seed: Option<u64>,
    /// Gaussian blur's standard deviation: 0 to 5.
    pub smoothing: f64,
    /// How strongly the rock melds with the world: 0 to 3.
    pub meld: f64,
}

impl Default for Rock {
    fn default() -> Self {
        // Smoothing 2 is the documented "standard"; the rest is Inferred.
        Rock { radius: 5, noise_radius: 4.0, noisiness: 0.5, seed: None, smoothing: 2.0, meld: 1.0 }
    }
}

impl Tool for Rock {
    fn id(&self) -> &'static str {
        "kaya"
    }
    fn name(&self) -> &'static str {
        "Kaya"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        match option {
            "yaricap" | "yarıçap" | "radius" => self.radius = number(value, 1, 24)?,
            "gurultu_yaricapi" | "noise_radius" => self.noise_radius = number(value, 1.0, 32.0)?,
            "gurultu" | "gürültü" | "noisiness" => self.noisiness = number(value, 0.0, 1.5)?,
            "tohum" | "seed" => {
                self.seed = if matches!(value, Some("rastgele") | Some("random")) {
                    None
                } else {
                    Some(number(value, 0u64, u64::MAX)?)
                }
            }
            "puruzsuzluk" | "pürüzsüzlük" | "smoothing" => self.smoothing = number(value, 0.0, 5.0)?,
            "kaynasma" | "kaynaşma" | "meld" => self.meld = number(value, 0.0, 3.0)?,
            _ => return unknown(option),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        vec![
            format!("yarıçap: {}", self.radius),
            format!("gürültü_yarıçapı: {}", self.noise_radius),
            format!("gürültü: {}", self.noisiness),
            match self.seed {
                Some(s) => format!("tohum: {s}"),
                None => "tohum: rastgele".to_string(),
            },
            format!("pürüzsüzlük: {}", self.smoothing),
            format!("kaynaşma: {}", self.meld),
        ]
    }
    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        if input.stroke.is_empty() {
            return msg("Vuruş yok.");
        }
        let (a, s) = normal_axis(input.face);
        let mut n = [0i32; 3];
        n[a] = s;
        let r = self.radius;
        // Inferred: without a seed, the stroke's start decides the rock.
        let seed = self.seed.unwrap_or_else(|| seed_of(input.stroke[0]));
        let reach = (f64::from(r) * (1.0 + self.noisiness)).ceil() as i32 + 1;
        let half = reach + blur_reach(self.smoothing);
        let side = (2 * half + 1) as usize;
        // Rock cells that stay when blurred, lowered a little by the meld
        // near the rock's own edge.
        let cutoff = (0.5 - 0.06 * self.meld) as f32;
        let mut out: BTreeMap<Pos, String> = BTreeMap::new();
        for p in thin(&input.stroke, (r / 2).max(1)) {
            let centre = add(p, mul(n, r / 2));
            let Some(mut g) = Grid::new(sub(centre, [half; 3]), [side; 3]) else {
                return msg("Kaya çok büyük.");
            };
            let density = |q: Pos| -> f64 {
                let d = dist(q, centre);
                let noise = noise3([f64::from(q[0]), f64::from(q[1]), f64::from(q[2])], self.noise_radius, seed);
                1.0 - d / (f64::from(r) + 0.5) + self.noisiness * noise
            };
            for x in 0..side {
                for y in 0..side {
                    for z in 0..side {
                        let q = g.pos([x, y, z]);
                        let v = if solid_at(world, q) || density(q) > 0.0 { 1.0 } else { 0.0 };
                        let i = g.index([x, y, z]);
                        g.data[i] = v;
                    }
                }
            }
            g.blur(self.smoothing);
            for x in 0..side {
                for y in 0..side {
                    for z in 0..side {
                        let q = g.pos([x, y, z]);
                        if dist(q, centre) > f64::from(reach) || !replaceable(world, q) {
                            continue;
                        }
                        let v = g.data[g.index([x, y, z])];
                        let d = density(q);
                        if v >= 0.5 || (d > -0.25 && v >= cutoff) {
                            out.insert(q, input.active.clone());
                        }
                    }
                }
            }
            if out.len() > MAX_BLOCKS {
                return too_many();
            }
        }
        done(out)
    }
}

// ---------------------------------------------------------------------------
// Weld and Melt
// ---------------------------------------------------------------------------

/// The brush's blocks and the solid mass around them, blurred.
fn prepare(
    world: &dyn Blocks,
    input: &Input,
    radius: i32,
    shape: Shape,
    sd: f64,
) -> Result<(BTreeSet<Pos>, Grid), Outcome> {
    if input.stroke.is_empty() {
        return Err(msg("Vuruş yok."));
    }
    if brush_too_big(input.stroke.len(), radius) {
        return Err(too_many());
    }
    let targets = stroke(&input.stroke, radius, shape);
    if targets.len() > MAX_BLOCKS {
        return Err(too_many());
    }
    let k = blur_reach(sd);
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    for p in &targets {
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    let mut dim = [0usize; 3];
    for a in 0..3 {
        dim[a] = (hi[a] - lo[a] + 1 + 2 * k) as usize;
    }
    let Some(mut g) = Grid::new(sub(lo, [k; 3]), dim) else {
        return Err(msg("Alan çok büyük."));
    };
    for x in 0..dim[0] {
        for y in 0..dim[1] {
            for z in 0..dim[2] {
                if solid_at(world, g.pos([x, y, z])) {
                    let i = g.index([x, y, z]);
                    g.data[i] = 1.0;
                }
            }
        }
    }
    g.blur(sd);
    Ok((targets, g))
}

/// Weld: adds the active block where a blur of the surrounding mass says the
/// surface should flare out (seams, concave corners).
pub struct Weld {
    pub radius: i32,
    pub shape: Shape,
    /// The blur's standard deviation in blocks: 0 to 5.
    pub strength: f64,
    /// The blurred mass a block needs to be added: 0 to 1; higher is harder.
    pub threshold: f64,
    /// May also replace other solid blocks.
    pub replace_solid: bool,
}

impl Default for Weld {
    fn default() -> Self {
        // Inferred: the documentation names the options without numbers.
        Weld { radius: 4, shape: Shape::Sphere, strength: 1.5, threshold: 0.45, replace_solid: false }
    }
}

impl Tool for Weld {
    fn id(&self) -> &'static str {
        "kaynak"
    }
    fn name(&self) -> &'static str {
        "Kaynak"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        match option {
            "yaricap" | "yarıçap" | "radius" => self.radius = number(value, 1, 32)?,
            "sekil" | "şekil" | "shape" => {
                self.shape = value.and_then(Shape::parse).ok_or("küre, küp veya sekizyüzlü bekleniyordu.")?
            }
            "guc" | "güç" | "yumusatma" | "strength" => self.strength = number(value, 0.0, 5.0)?,
            "esik" | "eşik" | "threshold" => self.threshold = number(value, 0.0, 1.0)?,
            "kati_degistir" | "replace_solid" => toggle(&mut self.replace_solid, value)?,
            _ => return unknown(option),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        vec![
            format!("yarıçap: {}", self.radius),
            format!("şekil: {}", self.shape.name()),
            format!("yumuşatma gücü: {}", self.strength),
            format!("yumuşatma eşiği: {}", self.threshold),
            format!("katı_değiştir: {}", on_off(self.replace_solid)),
        ]
    }
    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        let (targets, g) = match prepare(world, input, self.radius, self.shape, self.strength) {
            Ok(v) => v,
            Err(o) => return o,
        };
        let t = self.threshold as f32;
        let mut out = BTreeMap::new();
        for p in targets {
            if g.get(p) < t {
                continue;
            }
            let ok = if self.replace_solid {
                world.block(p).map_or(false, |b| block_string(&b) != input.active)
            } else {
                replaceable(world, p)
            };
            if ok {
                out.insert(p, input.active.clone());
            }
        }
        done(out)
    }
}

/// Melt: the opposite of Weld, removes blocks that stick out of the blurred
/// mass (edges, spikes) so surfaces erode.
pub struct Melt {
    pub radius: i32,
    pub shape: Shape,
    pub strength: f64,
    pub threshold: f64,
}

impl Default for Melt {
    fn default() -> Self {
        // Inferred, as for Weld.
        Melt { radius: 4, shape: Shape::Sphere, strength: 1.5, threshold: 0.45 }
    }
}

impl Tool for Melt {
    fn id(&self) -> &'static str {
        "eritme"
    }
    fn name(&self) -> &'static str {
        "Eritme"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        match option {
            "yaricap" | "yarıçap" | "radius" => self.radius = number(value, 1, 32)?,
            "sekil" | "şekil" | "shape" => {
                self.shape = value.and_then(Shape::parse).ok_or("küre, küp veya sekizyüzlü bekleniyordu.")?
            }
            "guc" | "güç" | "yumusatma" | "strength" => self.strength = number(value, 0.0, 5.0)?,
            "esik" | "eşik" | "threshold" => self.threshold = number(value, 0.0, 1.0)?,
            _ => return unknown(option),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        vec![
            format!("yarıçap: {}", self.radius),
            format!("şekil: {}", self.shape.name()),
            format!("yumuşatma gücü: {}", self.strength),
            format!("yumuşatma eşiği: {}", self.threshold),
        ]
    }
    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        let (targets, g) = match prepare(world, input, self.radius, self.shape, self.strength) {
            Ok(v) => v,
            Err(o) => return o,
        };
        let t = (1.0 - self.threshold) as f32;
        let mut out = BTreeMap::new();
        for p in targets {
            if filled(world, p) && g.get(p) <= t {
                out.insert(p, AIR.to_string());
            }
        }
        done(out)
    }
}

// ---------------------------------------------------------------------------
// Stamp
// ---------------------------------------------------------------------------

/// A quarter turn clockwise seen from above: east, south, west, north.
fn yaw_turn(mut x: i32, mut z: i32, turns: u32) -> (i32, i32) {
    for _ in 0..turns % 4 {
        let nx = -z;
        let nz = x;
        x = nx;
        z = nz;
    }
    (x, z)
}

/// A block's state turned and mirrored like the stamp: `facing` and `axis`
/// follow; other properties (stair shapes, rotation of banners) are kept.
fn rotate_state(state: &str, turns: u32, fx: bool, fz: bool) -> String {
    if state.is_empty() {
        return String::new();
    }
    const RING: [&str; 4] = ["east", "south", "west", "north"];
    let parts: Vec<String> = state
        .split(',')
        .map(|kv| match kv.split_once('=') {
            Some(("facing", v)) => {
                let mut v = v;
                if fx {
                    v = match v {
                        "east" => "west",
                        "west" => "east",
                        o => o,
                    };
                }
                if fz {
                    v = match v {
                        "north" => "south",
                        "south" => "north",
                        o => o,
                    };
                }
                match RING.iter().position(|r| *r == v) {
                    Some(i) => format!("facing={}", RING[(i + turns as usize) % 4]),
                    None => format!("facing={v}"),
                }
            }
            Some(("axis", v)) if turns % 2 == 1 => {
                let v2 = match v {
                    "x" => "z",
                    "z" => "x",
                    o => o,
                };
                format!("axis={v2}")
            }
            _ => kv.to_string(),
        })
        .collect();
    parts.join(",")
}

/// Stamp: puts copies of the stored structure (the selection, its blocks
/// read from the world) along the stroke, by chance and spacing.
pub struct Stamp {
    /// The chance a stamp is placed at a point: 1 always.
    pub base_chance: f64,
    /// The least distance between two stamp centres, as a part of the
    /// stamp's width (1 is 100%).
    pub min_spacing: f64,
    pub random_yaw: bool,
    /// A fixed turn in degrees (0, 90, 180, 270).
    pub yaw: i32,
    /// The way the stamp's top points (walls, ceilings).
    pub direction: Dir,
    pub random_flip: bool,
    pub keep_existing: bool,
    pub extend_foundation: bool,
    pub apply_gravity: bool,
}

impl Default for Stamp {
    fn default() -> Self {
        // Inferred: documented only as "1 means always" and "100%".
        Stamp {
            base_chance: 1.0,
            min_spacing: 1.0,
            random_yaw: false,
            yaw: 0,
            direction: Dir::Up,
            random_flip: false,
            keep_existing: false,
            extend_foundation: false,
            apply_gravity: false,
        }
    }
}

impl Tool for Stamp {
    fn id(&self) -> &'static str {
        "damga"
    }
    fn name(&self) -> &'static str {
        "Damga"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        match option {
            "sans" | "şans" | "base_chance" => self.base_chance = number(value, 0.0, 1.0)?,
            "aralik" | "aralık" | "min_spacing" => self.min_spacing = number(value, 0.0, 10.0)?,
            "rastgele_donus" | "random_yaw" => toggle(&mut self.random_yaw, value)?,
            "donus" | "dönüş" | "yaw" => {
                let v: i32 = number(value, 0, 359)?;
                if v % 90 != 0 {
                    return Err("Dönüş 0, 90, 180 veya 270 olmalı.".to_string());
                }
                self.yaw = v;
            }
            "yon" | "yön" | "direction" => {
                self.direction = value.and_then(Dir::parse).ok_or("yukarı, aşağı, kuzey, güney, doğu veya batı bekleniyordu.")?
            }
            "rastgele_cevir" | "random_flip" => toggle(&mut self.random_flip, value)?,
            "koru" | "keep_existing" => toggle(&mut self.keep_existing, value)?,
            "temel_uzat" | "extend_foundation" => toggle(&mut self.extend_foundation, value)?,
            "yercekimi" | "yerçekimi" | "apply_gravity" => toggle(&mut self.apply_gravity, value)?,
            _ => return unknown(option),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        vec![
            format!("şans: {}", self.base_chance),
            format!("aralık: {}", self.min_spacing),
            format!("rastgele_dönüş: {}", on_off(self.random_yaw)),
            format!("dönüş: {}", self.yaw),
            format!("yön: {}", self.direction.name()),
            format!("rastgele_çevir: {}", on_off(self.random_flip)),
            format!("koru: {}", on_off(self.keep_existing)),
            format!("temel_uzat: {}", on_off(self.extend_foundation)),
            format!("yerçekimi: {}", on_off(self.apply_gravity)),
        ]
    }
    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        if input.stroke.is_empty() {
            return msg("Vuruş yok.");
        }
        if input.selection.is_empty() {
            return msg("Damga için önce bir seçim yapın.");
        }
        if input.selection.len() > MAX_BLOCKS {
            return too_many();
        }
        let mut lo = [i32::MAX; 3];
        let mut hi = [i32::MIN; 3];
        for p in &input.selection {
            for a in 0..3 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
        }
        // The stamp's anchor: the middle of its base.
        let anchor = [(lo[0] + hi[0]).div_euclid(2), lo[1], (lo[2] + hi[2]).div_euclid(2)];
        let width = f64::from((hi[0] - lo[0] + 1).max(hi[2] - lo[2] + 1));
        let blocks: Vec<(Pos, Block)> = input
            .selection
            .iter()
            .filter_map(|p| world.block(*p).filter(|b| !b.air()).map(|b| (sub(*p, anchor), b)))
            .collect();
        if blocks.is_empty() {
            return msg("Seçimde blok yok.");
        }
        let up = self.direction.vector();
        let down = up.map(|v| -v);
        let mut rng = Rng::new(seed_of(input.stroke[0]));
        let mut centres: Vec<Pos> = Vec::new();
        let mut out: BTreeMap<Pos, String> = BTreeMap::new();
        for &p in &input.stroke {
            let crowded = centres.iter().any(|q| {
                let dx = f64::from(p[0] - q[0]);
                let dz = f64::from(p[2] - q[2]);
                (dx * dx + dz * dz).sqrt() < self.min_spacing * width
            });
            if crowded || rng.next_f64() >= self.base_chance {
                continue;
            }
            centres.push(p);
            let turns = if self.random_yaw { (rng.next_u64() % 4) as u32 } else { (self.yaw / 90) as u32 };
            let fx = self.random_flip && rng.next_u64() & 1 == 1;
            let fz = self.random_flip && rng.next_u64() & 1 == 1;
            let base = add(p, up);
            for (l, b) in &blocks {
                let (mut x, y, mut z) = (l[0], l[1], l[2]);
                if fx {
                    x = -x;
                }
                if fz {
                    z = -z;
                }
                (x, z) = yaw_turn(x, z, turns);
                let w = add(base, self.direction.map([x, y, z]));
                if self.keep_existing && filled(world, w) {
                    continue;
                }
                let turned = Block {
                    id: b.id.clone(),
                    state: rotate_state(&b.state, turns, fx, fz),
                    solid: b.solid,
                };
                let text = block_string(&turned);
                out.insert(w, text.clone());
                if self.extend_foundation && l[1] == 0 {
                    // The bottom layer is copied downwards until the ground.
                    let mut column = Vec::new();
                    let mut found = false;
                    for k in 1..=64 {
                        let q = add(w, mul(down, k));
                        if filled(world, q) {
                            found = true;
                            break;
                        }
                        column.push(q);
                    }
                    if found {
                        for q in column {
                            out.entry(q).or_insert_with(|| text.clone());
                        }
                    }
                }
            }
            if out.len() > MAX_BLOCKS {
                return too_many();
            }
        }
        if self.apply_gravity && !out.is_empty() {
            // The stamps fall together until one block rests on the world.
            let bottoms: Vec<Pos> = out
                .keys()
                .copied()
                .filter(|c| !out.contains_key(&[c[0], c[1] - 1, c[2]]))
                .collect();
            let mut drop = 0;
            while drop < 256 {
                if bottoms.iter().any(|c| filled(world, [c[0], c[1] - drop - 1, c[2]])) {
                    break;
                }
                drop += 1;
            }
            if drop > 0 {
                out = out.into_iter().map(|(c, b)| ([c[0], c[1] - drop, c[2]], b)).collect();
            }
        }
        done(out)
    }
}

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

/// The most characters of text one use writes.
const MAX_TEXT_CHARS: usize = 500;
/// Rows from one text line to the next (glyph, accents and a gap).
const LINE_HEIGHT: i32 = 14;

/// A 5x7 glyph: seven rows from the top, five bits each (bit 4 is the left
/// column). Letters are upper case; lower case is mapped to them.
fn glyph(c: char) -> Option<[u8; 7]> {
    Some(match c {
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1E, 0x01, 0x01, 0x0E, 0x01, 0x01, 0x1E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        ' ' => [0; 7],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x0C, 0x04, 0x08],
        '!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04],
        '?' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00],
        ':' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00],
        ';' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x04, 0x08],
        '\'' => [0x04, 0x04, 0x08, 0x00, 0x00, 0x00, 0x00],
        '"' => [0x0A, 0x0A, 0x00, 0x00, 0x00, 0x00, 0x00],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '/' => [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
        '=' => [0x00, 0x00, 0x1F, 0x00, 0x1F, 0x00, 0x00],
        '*' => [0x00, 0x15, 0x0E, 0x1F, 0x0E, 0x15, 0x00],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F],
        '#' => [0x0A, 0x0A, 0x1F, 0x0A, 0x1F, 0x0A, 0x0A],
        '%' => [0x18, 0x19, 0x02, 0x04, 0x08, 0x13, 0x03],
        '<' => [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02],
        '>' => [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08],
        _ => return None,
    })
}

/// A Turkish letter as its plain base glyph and the extra pixels (column,
/// height above the baseline) that mark it. Other characters have none.
fn accent(c: char) -> (char, &'static [(i32, i32)]) {
    match c {
        'Ç' => ('C', &[(2, -1), (1, -2)][..]),
        'Ş' => ('S', &[(2, -1), (1, -2)][..]),
        'Ğ' => ('G', &[(1, 9), (3, 9), (2, 8)][..]),
        'İ' => ('I', &[(2, 8)][..]),
        'Ö' => ('O', &[(1, 8), (3, 8)][..]),
        'Ü' => ('U', &[(1, 8), (3, 8)][..]),
        _ => (c, &[][..]),
    }
}

/// The lit pixels of `text` as (right, up) from the start of the baseline;
/// further lines go below.
fn layout(text: &str) -> BTreeSet<(i32, i32)> {
    let mut out = BTreeSet::new();
    let mut pen = 0;
    let mut line = 0;
    for ch in text.chars() {
        if ch == '\n' {
            pen = 0;
            line += 1;
            continue;
        }
        let upper = ch.to_uppercase().next().unwrap_or(ch);
        let (base, marks) = accent(upper);
        let rows = glyph(base).or_else(|| glyph('?')).unwrap_or([0; 7]);
        let origin = -line * LINE_HEIGHT;
        for (r, bits) in rows.iter().enumerate() {
            for c in 0..5i32 {
                if (*bits >> (4 - c)) & 1 == 1 {
                    out.insert((pen + c, origin + 6 - r as i32));
                }
            }
        }
        for &(c, v) in marks {
            out.insert((pen + c, origin + v));
        }
        pen += 6;
    }
    out
}

/// Text: block letters of the given text in the active block.
pub struct Text {
    /// Blocks per font pixel.
    pub size: i32,
    /// The side the text faces.
    pub direction: Dir,
}

impl Default for Text {
    fn default() -> Self {
        // Inferred: upright, readable from the south, at the font's own size.
        Text { size: 1, direction: Dir::South }
    }
}

impl Tool for Text {
    fn id(&self) -> &'static str {
        "yazi"
    }
    fn name(&self) -> &'static str {
        "Yazı"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        match option {
            "boyut" | "size" => self.size = number(value, 1, 16)?,
            "yon" | "yön" | "direction" => {
                self.direction = value.and_then(Dir::parse).ok_or("yukarı, aşağı, kuzey, güney, doğu veya batı bekleniyordu.")?
            }
            _ => return unknown(option),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        vec![format!("boyut: {}", self.size), format!("yön: {}", self.direction.name())]
    }
    fn apply(&self, _world: &dyn Blocks, input: &Input) -> Outcome {
        if input.stroke.is_empty() {
            return msg("Vuruş yok.");
        }
        let text: String = input.text.chars().take(MAX_TEXT_CHARS).collect();
        if text.trim().is_empty() {
            return msg("Yazı boş; önce metni girin.");
        }
        let pixels = layout(&text);
        let size = self.size.max(1);
        if pixels.len().saturating_mul((size * size) as usize) > MAX_BLOCKS {
            return too_many();
        }
        let (u, v) = self.direction.text_axes();
        // The text sits on the face that was hit.
        let origin = add(input.stroke[0], input.face);
        let mut out = BTreeMap::new();
        for &(pu, pv) in &pixels {
            for su in 0..size {
                for sv in 0..size {
                    let uu = pu * size + su;
                    let vv = pv * size + sv;
                    let q = [
                        origin[0] + u[0] * uu + v[0] * vv,
                        origin[1] + u[1] * uu + v[1] * vv,
                        origin[2] + u[2] * uu + v[2] * vv,
                    ];
                    out.insert(q, input.active.clone());
                }
            }
        }
        done(out)
    }
}

// ---------------------------------------------------------------------------
// Shape Tool
// ---------------------------------------------------------------------------

/// The Shape Tool's shapes: solid first, then flat ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Sphere,
    Cuboid,
    Octahedron,
    Supersphere,
    Cylinder,
    Tube,
    Cone,
    Pyramid,
    Torus,
    Dodecahedron,
    Icosahedron,
    Disk,
    Plane,
    Polygon,
    Superellipse,
    Spiral,
}

impl Kind {
    pub fn parse(word: &str) -> Option<Kind> {
        Some(match word {
            "kure" | "küre" | "sphere" => Kind::Sphere,
            "kupoid" | "kuboid" | "cuboid" => Kind::Cuboid,
            "sekizyuzlu" | "sekizyüzlü" | "octahedron" => Kind::Octahedron,
            "superkure" | "süperküre" | "supersphere" => Kind::Supersphere,
            "silindir" | "cylinder" => Kind::Cylinder,
            "tup" | "tüp" | "tube" => Kind::Tube,
            "koni" | "cone" => Kind::Cone,
            "piramit" | "pyramid" => Kind::Pyramid,
            "simit" | "torus" => Kind::Torus,
            "onikiyuzlu" | "onikiyüzlü" | "dodecahedron" => Kind::Dodecahedron,
            "yirmiyuzlu" | "yirmiyüzlü" | "icosahedron" => Kind::Icosahedron,
            "disk" => Kind::Disk,
            "duzlem" | "düzlem" | "plane" => Kind::Plane,
            "cokgen" | "çokgen" | "polygon" => Kind::Polygon,
            "superelips" | "süperelips" | "superellipse" => Kind::Superellipse,
            "sarmal" | "spiral" => Kind::Spiral,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Sphere => "küre",
            Kind::Cuboid => "küboid",
            Kind::Octahedron => "sekizyüzlü",
            Kind::Supersphere => "süperküre",
            Kind::Cylinder => "silindir",
            Kind::Tube => "tüp",
            Kind::Cone => "koni",
            Kind::Pyramid => "piramit",
            Kind::Torus => "simit",
            Kind::Dodecahedron => "onikiyüzlü",
            Kind::Icosahedron => "yirmiyüzlü",
            Kind::Disk => "disk",
            Kind::Plane => "düzlem",
            Kind::Polygon => "çokgen",
            Kind::Superellipse => "süperelips",
            Kind::Spiral => "sarmal",
        }
    }

    pub fn is_flat(self) -> bool {
        matches!(self, Kind::Disk | Kind::Plane | Kind::Polygon | Kind::Superellipse | Kind::Spiral)
    }
}

/// The face normals of a regular dodecahedron or icosahedron, unit length
/// (an icosahedron's faces point at a dodecahedron's corners, and back).
fn poly_normals(kind: Kind) -> Vec<[f64; 3]> {
    let phi = (1.0 + 5f64.sqrt()) / 2.0;
    let ip = 1.0 / phi;
    let signs = [-1.0f64, 1.0];
    let mut raw: Vec<[f64; 3]> = Vec::new();
    match kind {
        Kind::Icosahedron => {
            for a in signs {
                for b in signs {
                    for c in signs {
                        raw.push([a, b, c]);
                    }
                    raw.push([0.0, a * ip, b * phi]);
                    raw.push([a * ip, b * phi, 0.0]);
                    raw.push([b * phi, 0.0, a * ip]);
                }
            }
        }
        Kind::Dodecahedron => {
            for a in signs {
                for b in signs {
                    raw.push([0.0, a, b * phi]);
                    raw.push([a, b * phi, 0.0]);
                    raw.push([b * phi, 0.0, a]);
                }
            }
        }
        _ => {}
    }
    raw.into_iter()
        .map(|v| {
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            [v[0] / l, v[1] / l, v[2] / l]
        })
        .collect()
}

/// A vector turned back by the shape's rotation (yaw about y, then pitch
/// about x, then roll about z; angles in radians), into the shape's frame.
fn rot_inv(v: [f64; 3], yaw: f64, pitch: f64, roll: f64) -> [f64; 3] {
    let (mut x, mut y, mut z) = (v[0], v[1], v[2]);
    let (s, c) = (-yaw).sin_cos();
    let (nx, nz) = (x * c + z * s, -x * s + z * c);
    x = nx;
    z = nz;
    let (s, c) = (-pitch).sin_cos();
    let (ny, nz) = (y * c - z * s, y * s + z * c);
    y = ny;
    z = nz;
    let (s, c) = (-roll).sin_cos();
    let (nx, ny) = (x * c - y * s, x * s + y * c);
    x = nx;
    y = ny;
    [x, y, z]
}

/// Shape Tool: mathematical shapes of the active block, placed at the hit.
pub struct ShapeTool {
    pub kind: Kind,
    /// Diameter of every axis (when `separate` is off): 1 to 64.
    pub diameter: i32,
    /// Separate X, Y and Z sizes.
    pub separate: bool,
    pub size: [i32; 3],
    /// Rotation in degrees ("advanced").
    pub yaw: f64,
    pub pitch: f64,
    pub roll: f64,
    pub hollow: bool,
    /// The wall of a hollow shape, in blocks.
    pub thickness: i32,
    /// Melts the shape's surface (a blur).
    pub metaball: bool,
    /// The pivot is the shape's base, not its centre.
    pub base_pivot: bool,
    pub keep_existing: bool,
    /// Regular polygon: its sides.
    pub sides: i32,
    /// Supersphere and superellipse: the exponent (2 is a ball).
    pub exponent: f64,
    /// Tube: the hole's share of the radius.
    pub hole: f64,
    /// Torus: the tube's share of the radius.
    pub tube_ratio: f64,
    /// Spiral: how many turns.
    pub turns: f64,
}

impl Default for ShapeTool {
    fn default() -> Self {
        // Inferred: the documentation gives no sizes or shape parameters.
        ShapeTool {
            kind: Kind::Sphere,
            diameter: 11,
            separate: false,
            size: [11, 11, 11],
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            hollow: false,
            thickness: 1,
            metaball: false,
            base_pivot: false,
            keep_existing: false,
            sides: 6,
            exponent: 3.0,
            hole: 0.5,
            tube_ratio: 0.3,
            turns: 3.0,
        }
    }
}

impl ShapeTool {
    /// Whether the shape-space point `s` (blocks from the centre) is in the
    /// shape whose half sizes are `half`.
    fn inside(&self, s: [f64; 3], half: [f64; 3], normals: &[[f64; 3]]) -> bool {
        const EPS: f64 = 1e-9;
        let q = [s[0] / half[0], s[1] / half[1], s[2] / half[2]];
        let rxz2 = q[0] * q[0] + q[2] * q[2];
        let ay = q[1].abs();
        match self.kind {
            Kind::Sphere => rxz2 + q[1] * q[1] <= 1.0 + EPS,
            Kind::Cuboid => q.iter().all(|v| v.abs() <= 1.0 + EPS),
            Kind::Octahedron => q[0].abs() + ay + q[2].abs() <= 1.0 + EPS,
            Kind::Supersphere => q.iter().map(|v| v.abs().powf(self.exponent)).sum::<f64>() <= 1.0 + EPS,
            Kind::Cylinder => rxz2 <= 1.0 + EPS && ay <= 1.0 + EPS,
            Kind::Tube => rxz2 <= 1.0 + EPS && rxz2 >= self.hole * self.hole && ay <= 1.0 + EPS,
            Kind::Cone => {
                let r = (1.0 - q[1]) / 2.0;
                ay <= 1.0 + EPS && rxz2 <= r * r + EPS
            }
            Kind::Pyramid => {
                let r = (1.0 - q[1]) / 2.0;
                ay <= 1.0 + EPS && q[0].abs() <= r + EPS && q[2].abs() <= r + EPS
            }
            Kind::Torus => {
                let t = self.tube_ratio;
                let d = rxz2.sqrt() - (1.0 - t);
                (d * d) / (t * t) + q[1] * q[1] <= 1.0 + EPS
            }
            Kind::Dodecahedron | Kind::Icosahedron => {
                // The inscribed ball of either solid is 0.7947 of its
                // circumscribed one.
                normals.iter().all(|n| n[0] * q[0] + n[1] * q[1] + n[2] * q[2] <= 0.7947 + EPS)
            }
            Kind::Disk => s[1].abs() <= 0.5 + EPS && rxz2 <= 1.0 + EPS,
            Kind::Plane => s[1].abs() <= 0.5 + EPS && q[0].abs() <= 1.0 + EPS && q[2].abs() <= 1.0 + EPS,
            Kind::Superellipse => {
                s[1].abs() <= 0.5 + EPS
                    && q[0].abs().powf(self.exponent) + q[2].abs().powf(self.exponent) <= 1.0 + EPS
            }
            Kind::Polygon => {
                if s[1].abs() > 0.5 + EPS {
                    return false;
                }
                let r = rxz2.sqrt();
                if r <= EPS {
                    return true;
                }
                let n = f64::from(self.sides.max(3));
                let sector = std::f64::consts::TAU / n;
                let theta = q[2].atan2(q[0]);
                let a = theta.rem_euclid(sector) - sector / 2.0;
                r * a.cos() <= (std::f64::consts::PI / n).cos() + EPS
            }
            Kind::Spiral => {
                if s[1].abs() > 0.5 + EPS {
                    return false;
                }
                let r = rxz2.sqrt();
                if r > 1.0 + EPS {
                    return false;
                }
                // One arm of an Archimedean spiral: radius grows evenly
                // with the angle over `turns` turns.
                let pitch = 1.0 / self.turns.max(0.5);
                let theta = q[2].atan2(q[0]) / std::f64::consts::TAU;
                let f = (r / pitch - theta).rem_euclid(1.0);
                let d = f.min(1.0 - f);
                d * pitch * half[0].max(half[2]) <= 0.6
            }
        }
    }

    /// The blocks of the shape as offsets from where it is placed; None if
    /// it needs too big a scratch grid.
    fn offsets(&self) -> Option<BTreeSet<Pos>> {
        let flat = self.kind.is_flat();
        let sizes = if self.separate { self.size } else { [self.diameter; 3] };
        let sz = [sizes[0], if flat { 1 } else { sizes[1] }, sizes[2]];
        let half = [f64::from(sz[0]) / 2.0, f64::from(sz[1]) / 2.0, f64::from(sz[2]) / 2.0];
        let diag = f64::from(sz[0] * sz[0] + sz[1] * sz[1] + sz[2] * sz[2]).sqrt();
        let rb = (if self.base_pivot { diag } else { diag / 2.0 }).ceil() as i32 + 2;
        let pad = if self.metaball { 4 } else { 1 };
        let h = rb + pad;
        let side = (2 * h + 1) as usize;
        let mut g = Grid::new([-h; 3], [side; 3])?;
        let (yaw, pitch, roll) = (self.yaw.to_radians(), self.pitch.to_radians(), self.roll.to_radians());
        let normals = poly_normals(self.kind);
        // An even size has no middle block: its centre is half a block off.
        let shift = [
            if sz[0] % 2 == 0 { 0.5 } else { 0.0 },
            if self.base_pivot {
                -f64::from(sz[1] - 1) / 2.0
            } else if sz[1] % 2 == 0 {
                0.5
            } else {
                0.0
            },
            if sz[2] % 2 == 0 { 0.5 } else { 0.0 },
        ];
        for x in 0..side {
            for y in 0..side {
                for z in 0..side {
                    let p = g.pos([x, y, z]);
                    let r = rot_inv([f64::from(p[0]), f64::from(p[1]), f64::from(p[2])], yaw, pitch, roll);
                    let s = [r[0] + shift[0], r[1] + shift[1], r[2] + shift[2]];
                    if self.inside(s, half, &normals) {
                        let i = g.index([x, y, z]);
                        g.data[i] = 1.0;
                    }
                }
            }
        }
        if self.metaball {
            g.blur(1.5);
            for v in g.data.iter_mut() {
                *v = if *v >= 0.4 { 1.0 } else { 0.0 };
            }
        }
        let t = self.thickness.max(1);
        let mut out = BTreeSet::new();
        for x in 0..side {
            for y in 0..side {
                for z in 0..side {
                    if g.data[g.index([x, y, z])] < 0.5 {
                        continue;
                    }
                    let p = g.pos([x, y, z]);
                    if self.hollow && !DIRS.iter().any(|d| g.get(add(p, mul(*d, t))) < 0.5) {
                        continue;
                    }
                    out.insert(p);
                }
            }
        }
        Some(out)
    }
}

impl Tool for ShapeTool {
    fn id(&self) -> &'static str {
        "sekil"
    }
    fn name(&self) -> &'static str {
        "Şekil Aracı"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        match option {
            "kategori" | "category" => match value {
                Some("duz" | "düz" | "flat") => {
                    if !self.kind.is_flat() {
                        self.kind = Kind::Disk;
                    }
                }
                Some("kati" | "katı" | "solid") => {
                    if self.kind.is_flat() {
                        self.kind = Kind::Sphere;
                    }
                }
                _ => return Err("katı veya düz bekleniyordu.".to_string()),
            },
            "sekil" | "şekil" | "shape" => self.kind = value.and_then(Kind::parse).ok_or("Bilinmeyen şekil.")?,
            "cap" | "çap" | "diameter" => self.diameter = number(value, 1, 64)?,
            "xyz" | "ayri_xyz" => toggle(&mut self.separate, value)?,
            "x" => self.size[0] = number(value, 1, 64)?,
            "y" => self.size[1] = number(value, 1, 64)?,
            "z" => self.size[2] = number(value, 1, 64)?,
            "yaw" | "sapma" => self.yaw = number(value, -360.0, 360.0)?,
            "pitch" | "yunuslama" => self.pitch = number(value, -360.0, 360.0)?,
            "roll" | "yatis" | "yatış" => self.roll = number(value, -360.0, 360.0)?,
            "oyuk" | "hollow" => toggle(&mut self.hollow, value)?,
            "kalinlik" | "kalınlık" | "thickness" => self.thickness = number(value, 1, 16)?,
            "metaball" | "erime" => toggle(&mut self.metaball, value)?,
            "taban" | "base" => toggle(&mut self.base_pivot, value)?,
            "koru" | "keep_existing" => toggle(&mut self.keep_existing, value)?,
            "kenar" | "sides" => self.sides = number(value, 3, 64)?,
            "us" | "üs" | "exponent" => self.exponent = number(value, 0.5, 16.0)?,
            "delik" | "hole" => self.hole = number(value, 0.0, 0.95)?,
            "halka" | "tube_ratio" => self.tube_ratio = number(value, 0.05, 1.0)?,
            "tur" | "tür" | "turns" => self.turns = number(value, 0.5, 20.0)?,
            _ => return unknown(option),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        vec![
            format!("kategori: {}", if self.kind.is_flat() { "düz" } else { "katı" }),
            format!("şekil: {}", self.kind.name()),
            format!("çap: {}", self.diameter),
            format!("ayrı xyz: {} ({} x {} x {})", on_off(self.separate), self.size[0], self.size[1], self.size[2]),
            format!("açılar (sapma, yunuslama, yatış): {} {} {}", self.yaw, self.pitch, self.roll),
            format!("oyuk: {} (kalınlık {})", on_off(self.hollow), self.thickness),
            format!("metaball: {}", on_off(self.metaball)),
            format!("taban: {}", on_off(self.base_pivot)),
            format!("koru: {}", on_off(self.keep_existing)),
            format!("kenar: {}, üs: {}, delik: {}, halka: {}, tur: {}", self.sides, self.exponent, self.hole, self.tube_ratio, self.turns),
        ]
    }
    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        if input.stroke.is_empty() {
            return msg("Vuruş yok.");
        }
        let Some(cells) = self.offsets() else {
            return msg("Şekil çok büyük.");
        };
        if cells.len() > MAX_BLOCKS {
            return too_many();
        }
        let origin = add(input.stroke[0], input.face);
        // The second action carves the shape out instead.
        let block = if input.secondary { AIR.to_string() } else { input.active.clone() };
        let mut out = BTreeMap::new();
        for o in cells {
            let p = add(origin, o);
            if self.keep_existing && !input.secondary && filled(world, p) {
                continue;
            }
            out.insert(p, block.clone());
        }
        done(out)
    }
}

// ---------------------------------------------------------------------------
// Path
// ---------------------------------------------------------------------------

/// How a path joins its points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathKind {
    /// A line one block wide.
    Bresenham,
    /// A line with a radius.
    Dda,
    /// A hanging rope between the points.
    Catenary,
    /// A smooth curve through every point.
    Catmull,
    /// A smooth curve that only keeps the first and last point.
    Bezier,
}

impl PathKind {
    pub fn parse(word: &str) -> Option<PathKind> {
        Some(match word {
            "bresenham" => PathKind::Bresenham,
            "dda" => PathKind::Dda,
            "katener" | "catenary" => PathKind::Catenary,
            "catmull" | "catmull-rom" => PathKind::Catmull,
            "bezier" | "bézier" => PathKind::Bezier,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            PathKind::Bresenham => "bresenham",
            PathKind::Dda => "dda",
            PathKind::Catenary => "katener",
            PathKind::Catmull => "catmull",
            PathKind::Bezier => "bezier",
        }
    }
}

/// The brush a path draws with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathShape {
    Sphere,
    /// A disk lying flat.
    Flat,
    /// The selection (the clipboard) as the brush.
    Clipboard,
}

/// Path: a line or a curve through the stroke's points.
pub struct PathTool {
    pub kind: PathKind,
    pub looped: bool,
    pub shape: PathShape,
    pub radius: i32,
    pub keep_existing: bool,
    pub extend_to_ground: bool,
    /// Catenary: how far it sags, as a part of the span.
    pub slack: f64,
    /// Catenary: sags upwards.
    pub inverted: bool,
}

impl Default for PathTool {
    fn default() -> Self {
        // Inferred: the documentation names a radius of one or more as best
        // for stairs; slack has no documented number.
        PathTool {
            kind: PathKind::Catmull,
            looped: false,
            shape: PathShape::Sphere,
            radius: 1,
            keep_existing: false,
            extend_to_ground: false,
            slack: 0.3,
            inverted: false,
        }
    }
}

/// How many steps to sample a stretch of `len` blocks in (even, at least 2).
fn steps(len: f64) -> usize {
    let n = ((len * 2.0).ceil() as usize).max(2);
    n + n % 2
}

fn lerp3(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// The blocks of a straight line, one per step of its longest axis.
fn line_cells(a: Pos, b: Pos) -> Vec<Pos> {
    let d = sub(b, a);
    let n = d.iter().map(|v| v.abs()).max().unwrap_or(0);
    if n == 0 {
        return vec![a];
    }
    (0..=n)
        .map(|i| {
            let mut p = a;
            for k in 0..3 {
                p[k] = a[k] + (f64::from(d[k]) * f64::from(i) / f64::from(n)).round() as i32;
            }
            p
        })
        .collect()
}

/// A uniform Catmull-Rom curve between `p1` and `p2` at `t`.
fn catmull(p0: [f64; 3], p1: [f64; 3], p2: [f64; 3], p3: [f64; 3], t: f64) -> [f64; 3] {
    let t2 = t * t;
    let t3 = t2 * t;
    let mut out = [0.0; 3];
    for k in 0..3 {
        out[k] = 0.5
            * (2.0 * p1[k]
                + (-p0[k] + p2[k]) * t
                + (2.0 * p0[k] - 5.0 * p1[k] + 4.0 * p2[k] - p3[k]) * t2
                + (-p0[k] + 3.0 * p1[k] - 3.0 * p2[k] + p3[k]) * t3);
    }
    out
}

/// A Bezier curve of all the control points at `t` (de Casteljau).
fn de_casteljau(ctrl: &[[f64; 3]], t: f64) -> [f64; 3] {
    let mut v = ctrl.to_vec();
    while v.len() > 1 {
        for i in 0..v.len() - 1 {
            v[i] = lerp3(v[i], v[i + 1], t);
        }
        v.pop();
    }
    v[0]
}

impl PathTool {
    /// The blocks the path's centre line passes through.
    fn centres(&self, pts: &[Pos]) -> BTreeSet<Pos> {
        let mut out = BTreeSet::new();
        let n = pts.len();
        if n == 1 {
            out.insert(pts[0]);
            return out;
        }
        let f: Vec<[f64; 3]> = pts.iter().map(|p| p.map(f64::from)).collect();
        let segs = if self.looped { n } else { n - 1 };
        match self.kind {
            PathKind::Bresenham => {
                for i in 0..segs {
                    out.extend(line_cells(pts[i], pts[(i + 1) % n]));
                }
            }
            PathKind::Dda => {
                for i in 0..segs {
                    let (a, b) = (f[i], f[(i + 1) % n]);
                    let m = steps(dist3(a, b));
                    for s in 0..=m {
                        out.insert(round_pos(lerp3(a, b, s as f64 / m as f64)));
                    }
                }
            }
            PathKind::Catenary => {
                const K: f64 = 2.0;
                let sign = if self.inverted { 1.0 } else { -1.0 };
                for i in 0..segs {
                    let (a, b) = (f[i], f[(i + 1) % n]);
                    let len = dist3(a, b);
                    let sag = self.slack * len;
                    let m = steps(len);
                    for s in 0..=m {
                        let t = s as f64 / m as f64;
                        // 0 at both ends, 1 in the middle.
                        let dip = (K.cosh() - (K * (2.0 * t - 1.0)).cosh()) / (K.cosh() - 1.0);
                        let mut q = lerp3(a, b, t);
                        q[1] += sign * sag * dip;
                        out.insert(round_pos(q));
                    }
                }
            }
            PathKind::Catmull => {
                let last = n as i64 - 1;
                for i in 0..segs {
                    let idx = |j: i64| -> usize {
                        if self.looped {
                            j.rem_euclid(n as i64) as usize
                        } else {
                            j.clamp(0, last) as usize
                        }
                    };
                    let i = i as i64;
                    let (p0, p1, p2, p3) = (f[idx(i - 1)], f[idx(i)], f[idx(i + 1)], f[idx(i + 2)]);
                    let m = steps(dist3(p1, p2));
                    for s in 0..=m {
                        out.insert(round_pos(catmull(p0, p1, p2, p3, s as f64 / m as f64)));
                    }
                }
            }
            PathKind::Bezier => {
                let mut ctrl = f.clone();
                if self.looped {
                    ctrl.push(f[0]);
                }
                let len: f64 = ctrl.windows(2).map(|w| dist3(w[0], w[1])).sum();
                let m = steps(len);
                for s in 0..=m {
                    out.insert(round_pos(de_casteljau(&ctrl, s as f64 / m as f64)));
                }
            }
        }
        out
    }

    /// The brush's offsets around each centre.
    fn brush_offsets(&self, input: &Input) -> Vec<Pos> {
        if self.kind == PathKind::Bresenham {
            return vec![[0, 0, 0]];
        }
        let r = self.radius.max(0);
        let sphere = || brush([0, 0, 0], r, Shape::Sphere);
        match self.shape {
            PathShape::Sphere => sphere(),
            PathShape::Flat => {
                let mut v = Vec::new();
                for dx in -r..=r {
                    for dz in -r..=r {
                        if 4 * (dx * dx + dz * dz) <= (2 * r + 1) * (2 * r + 1) {
                            v.push([dx, 0, dz]);
                        }
                    }
                }
                v
            }
            PathShape::Clipboard => {
                if input.selection.is_empty() {
                    return sphere();
                }
                let mut lo = [i32::MAX; 3];
                let mut hi = [i32::MIN; 3];
                for p in &input.selection {
                    for a in 0..3 {
                        lo[a] = lo[a].min(p[a]);
                        hi[a] = hi[a].max(p[a]);
                    }
                }
                let mid = [
                    (lo[0] + hi[0]).div_euclid(2),
                    (lo[1] + hi[1]).div_euclid(2),
                    (lo[2] + hi[2]).div_euclid(2),
                ];
                input.selection.iter().map(|p| sub(*p, mid)).collect()
            }
        }
    }
}

impl Tool for PathTool {
    fn id(&self) -> &'static str {
        "yol"
    }
    fn name(&self) -> &'static str {
        "Yol"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        match option {
            "tur" | "tür" | "type" => {
                self.kind = value.and_then(PathKind::parse).ok_or("bresenham, dda, katener, catmull veya bezier bekleniyordu.")?
            }
            "dongu" | "döngü" | "looped" => toggle(&mut self.looped, value)?,
            "sekil" | "şekil" | "shape" => {
                self.shape = match value {
                    Some("kure" | "küre" | "sphere") => PathShape::Sphere,
                    Some("duz" | "düz" | "flat") => PathShape::Flat,
                    Some("pano" | "clipboard") => PathShape::Clipboard,
                    _ => return Err("küre, düz veya pano bekleniyordu.".to_string()),
                }
            }
            "yaricap" | "yarıçap" | "radius" => self.radius = number(value, 0, 16)?,
            "koru" | "keep_existing" => toggle(&mut self.keep_existing, value)?,
            "zemin" | "extend_to_ground" => toggle(&mut self.extend_to_ground, value)?,
            "sarkma" | "slack" => self.slack = number(value, 0.0, 3.0)?,
            "ters" | "inverted" => toggle(&mut self.inverted, value)?,
            _ => return unknown(option),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        vec![
            format!("tür: {}", self.kind.name()),
            format!("döngü: {}", on_off(self.looped)),
            format!(
                "şekil: {}",
                match self.shape {
                    PathShape::Sphere => "küre",
                    PathShape::Flat => "düz",
                    PathShape::Clipboard => "pano",
                }
            ),
            format!("yarıçap: {}", self.radius),
            format!("koru: {}", on_off(self.keep_existing)),
            format!("zemine uzat: {}", on_off(self.extend_to_ground)),
            format!("sarkma: {}", self.slack),
            format!("ters: {}", on_off(self.inverted)),
        ]
    }
    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        if input.stroke.is_empty() {
            return msg("Vuruş yok.");
        }
        let centres = self.centres(&input.stroke);
        let offs = self.brush_offsets(input);
        if centres.len().saturating_mul(offs.len()) > 64_000_000 {
            return too_many();
        }
        let mut cells: BTreeSet<Pos> = BTreeSet::new();
        for c in &centres {
            for o in &offs {
                cells.insert(add(*c, *o));
            }
            if cells.len() > MAX_BLOCKS {
                return too_many();
            }
        }
        if self.extend_to_ground {
            // Every column's lowest block is carried down to solid ground.
            let snapshot: Vec<Pos> = cells.iter().copied().collect();
            for c in snapshot {
                if cells.contains(&[c[0], c[1] - 1, c[2]]) {
                    continue;
                }
                let mut column = Vec::new();
                let mut found = false;
                for k in 1..=128 {
                    let q = [c[0], c[1] - k, c[2]];
                    if cells.contains(&q) || solid_at(world, q) {
                        found = true;
                        break;
                    }
                    column.push(q);
                }
                if found {
                    cells.extend(column);
                }
            }
            if cells.len() > MAX_BLOCKS {
                return too_many();
            }
        }
        let mut out = BTreeMap::new();
        for c in cells {
            if self.keep_existing && filled(world, c) {
                continue;
            }
            out.insert(c, input.active.clone());
        }
        done(out)
    }
}

/// The tools of this module, in the order of Axiom's documentation.
pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(Freehand::default()),
        Box::new(Sculpt::default()),
        Box::new(Rock::default()),
        Box::new(Weld::default()),
        Box::new(Melt::default()),
        Box::new(Stamp::default()),
        Box::new(Text::default()),
        Box::new(ShapeTool::default()),
        Box::new(PathTool::default()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    struct W {
        m: BTreeMap<Pos, Block>,
    }

    impl Blocks for W {
        fn block(&self, at: Pos) -> Option<Block> {
            Some(self.m.get(&at).cloned().unwrap_or_else(|| blk("minecraft:air", false)))
        }
    }

    fn blk(id: &str, solid: bool) -> Block {
        Block { id: id.to_string(), state: String::new(), solid }
    }

    impl W {
        fn new() -> W {
            W { m: BTreeMap::new() }
        }
        fn put(&mut self, p: Pos, id: &str) {
            self.m.insert(p, blk(id, true));
        }
        /// A thick stone floor under y = 0 (y from -4 to 0).
        fn ground(&mut self, half: i32) {
            for x in -half..=half {
                for z in -half..=half {
                    for y in -4..=0 {
                        self.put([x, y, z], "minecraft:stone");
                    }
                }
            }
        }
    }

    fn input(stroke: Vec<Pos>) -> Input {
        Input { stroke, active: "minecraft:stone".to_string(), ..Default::default() }
    }

    fn edits(o: Outcome) -> BTreeMap<Pos, String> {
        match o {
            Outcome::Edit(m) => m,
            other => panic!("edit expected, got {other:?}"),
        }
    }

    fn set_all(t: &mut dyn Tool, opts: &[(&str, Option<&str>)]) {
        for (k, v) in opts {
            t.set(k, *v).unwrap();
        }
    }

    #[test]
    fn tool_list_and_ids() {
        let ids: Vec<&str> = tools().iter().map(|t| t.id()).collect();
        assert_eq!(ids, ["ciz", "heykel", "kaya", "kaynak", "eritme", "damga", "yazi", "sekil", "yol"]);
        for t in tools() {
            assert!(!t.describe().is_empty());
        }
    }

    #[test]
    fn options_are_checked() {
        let mut t = Freehand::default();
        assert!(t.set("yaricap", Some("99")).is_err());
        assert!(t.set("bilinmeyen", None).is_err());
        assert!(t.set("sekil", Some("kup")).is_ok());
        assert_eq!(t.shape, Shape::Cube);
    }

    #[test]
    fn freehand_fills_and_erases() {
        let w = W::new();
        let mut t = Freehand::default();
        set_all(&mut t, &[("yaricap", Some("1")), ("sekil", Some("kup"))]);
        let out = edits(t.apply(&w, &input(vec![[0, 0, 0]])));
        assert_eq!(out.len(), 27);
        assert!(out.values().all(|v| v == "minecraft:stone"));
        let mut inp = input(vec![[0, 0, 0]]);
        inp.secondary = true;
        assert!(edits(t.apply(&w, &inp)).values().all(|v| v == AIR));
        assert!(matches!(t.apply(&w, &input(vec![])), Outcome::Message(_)));
    }

    #[test]
    fn sculpt_pulls_out_and_presses_in() {
        let mut w = W::new();
        w.ground(8);
        let mut inp = input(vec![[0, 0, 0]]);
        inp.face = [0, 1, 0];
        let mut t = Sculpt::default();
        t.set("yaricap", Some("3")).unwrap();
        let out = edits(t.apply(&w, &inp));
        assert!(out.contains_key(&[0, 1, 0]));
        assert!(out.keys().all(|p| p[1] >= 1));
        assert!(out.values().all(|v| v == "minecraft:stone"));
        // The second action presses the surface in.
        inp.secondary = true;
        let dug = edits(t.apply(&w, &inp));
        assert_eq!(dug.get(&[0, 0, 0]).map(String::as_str), Some(AIR));
        assert!(dug.keys().all(|p| p[1] <= 0));
    }

    #[test]
    fn rock_is_deterministic_and_sits_on_the_surface() {
        let mut w = W::new();
        w.ground(10);
        let mut inp = input(vec![[0, 0, 0]]);
        inp.face = [0, 1, 0];
        let mut t = Rock::default();
        set_all(&mut t, &[("yaricap", Some("4")), ("tohum", Some("5")), ("gurultu", Some("0"))]);
        let a = edits(t.apply(&w, &inp));
        let b = edits(t.apply(&w, &inp));
        assert_eq!(a, b);
        assert!(a.contains_key(&[0, 2, 0]));
        assert!(a.keys().all(|p| p[1] >= 1));
        assert!(a.len() > 20);
    }

    #[test]
    fn weld_fills_corners_but_not_flat_ground() {
        let mut w = W::new();
        w.ground(8);
        for x in 2..=8 {
            for z in -8..=8 {
                for y in 1..=6 {
                    w.put([x, y, z], "minecraft:stone");
                }
            }
        }
        let mut t = Weld::default();
        set_all(&mut t, &[("yaricap", Some("2")), ("sekil", Some("kup"))]);
        let out = edits(t.apply(&w, &input(vec![[1, 1, 0]])));
        assert!(out.contains_key(&[1, 1, 0]));
        assert!(!out.contains_key(&[-1, 1, 0]));
    }

    #[test]
    fn melt_erodes_corners_and_keeps_the_inside() {
        let mut w = W::new();
        for x in 0..=6 {
            for y in 0..=6 {
                for z in 0..=6 {
                    w.put([x, y, z], "minecraft:stone");
                }
            }
        }
        let mut t = Melt::default();
        set_all(&mut t, &[("yaricap", Some("1")), ("sekil", Some("kup"))]);
        let out = edits(t.apply(&w, &input(vec![[0, 0, 0]])));
        assert_eq!(out.get(&[0, 0, 0]).map(String::as_str), Some(AIR));
        assert!(!out.contains_key(&[1, 1, 1]));
    }

    #[test]
    fn stamp_copies_turns_and_extends() {
        let mut w = W::new();
        w.put([50, 50, 50], "minecraft:stone");
        w.m.insert(
            [51, 50, 50],
            Block { id: "minecraft:oak_stairs".to_string(), state: "facing=east".to_string(), solid: true },
        );
        let mut inp = input(vec![[10, 5, 10]]);
        inp.selection = [[50, 50, 50], [51, 50, 50]].into_iter().collect();
        let mut t = Stamp::default();
        let out = edits(t.apply(&w, &inp));
        assert_eq!(out.len(), 2);
        assert_eq!(out[&[10, 6, 10]], "minecraft:stone");
        assert_eq!(out[&[11, 6, 10]], "minecraft:oak_stairs[facing=east]");
        // A quarter turn: the second block moves to +z and faces south.
        t.set("donus", Some("90")).unwrap();
        let out = edits(t.apply(&w, &inp));
        assert_eq!(out[&[10, 6, 11]], "minecraft:oak_stairs[facing=south]");
        // No chance, no stamp.
        t.set("sans", Some("0")).unwrap();
        assert!(matches!(t.apply(&w, &inp), Outcome::Message(_)));
        // No selection, no stamp.
        assert!(matches!(Stamp::default().apply(&w, &input(vec![[0, 0, 0]])), Outcome::Message(_)));
    }

    #[test]
    fn stamp_foundation_gravity_and_keep() {
        let mut w = W::new();
        w.ground(3);
        w.put([50, 50, 50], "minecraft:stone");
        let mut inp = input(vec![[0, 5, 0]]);
        inp.selection = [[50, 50, 50]].into_iter().collect();
        let mut t = Stamp::default();
        t.set("temel_uzat", None).unwrap();
        // The stamp sits at y 6, its foundation fills y 1..=5 down to the floor.
        assert_eq!(edits(t.apply(&w, &inp)).len(), 6);
        let mut g = Stamp::default();
        g.set("yercekimi", None).unwrap();
        let fell = edits(g.apply(&w, &inp));
        assert!(fell.contains_key(&[0, 1, 0]));
        // Keep existing: stamping into the floor changes nothing.
        let mut k = Stamp::default();
        k.set("koru", None).unwrap();
        assert!(matches!(k.apply(&w, &input_at([0, -1, 0], &inp)), Outcome::Message(_)));
    }

    fn input_at(p: Pos, base: &Input) -> Input {
        let mut i = base.clone();
        // The stamp's base lands one above the stroke point: y = 0 is stone.
        i.stroke = vec![[p[0], p[1] - 1, p[2]]];
        i
    }

    #[test]
    fn directions_turn_up_to_their_vector() {
        for d in [Dir::Up, Dir::Down, Dir::North, Dir::South, Dir::East, Dir::West] {
            assert_eq!(d.map([0, 1, 0]), d.vector());
            assert_eq!(Dir::parse(d.name()).map(|x| x.vector()), Some(d.vector()));
        }
    }

    #[test]
    fn font_covers_the_alphabet_digits_and_turkish() {
        for c in ('A'..='Z').chain('0'..='9') {
            assert!(glyph(c).is_some(), "{c}");
        }
        for c in "Ç Ğ İ Ö Ş Ü".split(' ').map(|s| s.chars().next().unwrap()) {
            assert!(glyph(accent(c).0).is_some());
            assert!(!accent(c).1.is_empty());
        }
        assert_eq!(layout("ç"), layout("Ç"));
        assert_eq!(layout("a"), layout("A"));
    }

    #[test]
    fn text_draws_letters() {
        let w = W::new();
        let mut inp = input(vec![[0, 0, 0]]);
        inp.text = "A".to_string();
        let t = Text::default();
        let out = edits(t.apply(&w, &inp));
        assert_eq!(out.len(), 18);
        // South facing: upright, the top row at y 6, the left edge at x 0.
        assert_eq!(out.keys().map(|p| p[1]).max(), Some(6));
        assert_eq!(out.keys().map(|p| p[0]).min(), Some(0));
        assert!(out.keys().all(|p| p[2] == 0));
        let mut big = Text::default();
        big.set("boyut", Some("2")).unwrap();
        assert_eq!(edits(big.apply(&w, &inp)).len(), 72);
        // Accents add pixels: I has 11, İ has 12; Ü has 17.
        inp.text = "İ".to_string();
        assert_eq!(edits(t.apply(&w, &inp)).len(), 12);
        inp.text = "Ü".to_string();
        assert_eq!(edits(t.apply(&w, &inp)).len(), 17);
        // Flat on the ground: the text lies on the face that was hit.
        let mut flat = Text::default();
        flat.set("yon", Some("yukari")).unwrap();
        inp.text = "A".to_string();
        inp.face = [0, 1, 0];
        let out = edits(flat.apply(&w, &inp));
        assert!(out.keys().all(|p| p[1] == 1 && p[2] <= 0 && p[2] >= -6));
        // Empty text is a message.
        inp.text = "   ".to_string();
        assert!(matches!(t.apply(&w, &inp), Outcome::Message(_)));
    }

    #[test]
    fn shape_solids() {
        let w = W::new();
        let inp = input(vec![[0, 0, 0]]);
        let mut cube = ShapeTool::default();
        set_all(&mut cube, &[("sekil", Some("kupoid")), ("cap", Some("3"))]);
        assert_eq!(edits(cube.apply(&w, &inp)).len(), 27);
        cube.set("oyuk", None).unwrap();
        assert_eq!(edits(cube.apply(&w, &inp)).len(), 26);
        let mut even = ShapeTool::default();
        set_all(&mut even, &[("sekil", Some("kupoid")), ("cap", Some("4"))]);
        assert_eq!(edits(even.apply(&w, &inp)).len(), 64);

        let mut ball = ShapeTool::default();
        ball.set("cap", Some("9")).unwrap();
        let solid = edits(ball.apply(&w, &inp));
        assert!(solid.contains_key(&[4, 0, 0]));
        assert!(!solid.contains_key(&[5, 0, 0]));
        assert!(solid.len() > 250 && solid.len() < 729);
        ball.set("oyuk", None).unwrap();
        let shell = edits(ball.apply(&w, &inp));
        assert!(shell.len() < solid.len());
        assert!(!shell.contains_key(&[0, 0, 0]));

        let mut torus = ShapeTool::default();
        torus.set("sekil", Some("simit")).unwrap();
        let o = edits(torus.apply(&w, &inp));
        assert!(!o.contains_key(&[0, 0, 0]));
        assert!(o.contains_key(&[4, 0, 0]));

        let mut cone = ShapeTool::default();
        set_all(&mut cone, &[("sekil", Some("koni")), ("cap", Some("9"))]);
        let o = edits(cone.apply(&w, &inp));
        let layer = |y: i32| o.keys().filter(|p| p[1] == y).count();
        assert!(layer(-4) > layer(4));
        assert_eq!(layer(4), 1);

        for name in ["silindir", "tup", "piramit", "onikiyuzlu", "yirmiyuzlu", "supersphere", "sekizyuzlu"] {
            let mut t = ShapeTool::default();
            set_all(&mut t, &[("sekil", Some(name)), ("cap", Some("9"))]);
            assert!(!edits(t.apply(&w, &inp)).is_empty(), "{name}");
        }
    }

    #[test]
    fn shape_flat_pivot_rotation_and_metaball() {
        let w = W::new();
        let inp = input(vec![[0, 0, 0]]);
        let mut disk = ShapeTool::default();
        set_all(&mut disk, &[("kategori", Some("duz")), ("cap", Some("9"))]);
        let o = edits(disk.apply(&w, &inp));
        assert!(o.keys().all(|p| p[1] == 0));
        assert!(o.contains_key(&[4, 0, 0]));

        for name in ["duzlem", "cokgen", "superelips", "sarmal"] {
            let mut t = ShapeTool::default();
            set_all(&mut t, &[("sekil", Some(name)), ("cap", Some("15"))]);
            assert!(edits(t.apply(&w, &inp)).keys().all(|p| p[1] == 0), "{name}");
        }

        let mut base = ShapeTool::default();
        set_all(&mut base, &[("sekil", Some("kupoid")), ("cap", Some("3")), ("taban", None)]);
        let o = edits(base.apply(&w, &inp));
        assert_eq!(o.keys().map(|p| p[1]).min(), Some(0));
        assert_eq!(o.keys().map(|p| p[1]).max(), Some(2));

        // A bar along x, turned 90 degrees about y, lies along z.
        let mut bar = ShapeTool::default();
        set_all(
            &mut bar,
            &[("sekil", Some("kupoid")), ("xyz", None), ("x", Some("7")), ("y", Some("1")), ("z", Some("1")), ("yaw", Some("90"))],
        );
        let o = edits(bar.apply(&w, &inp));
        let want: BTreeSet<Pos> = (-3..=3).map(|z| [0, 0, z]).collect();
        assert_eq!(o.keys().copied().collect::<BTreeSet<Pos>>(), want);

        // A melted cube loses its corners.
        let mut melted = ShapeTool::default();
        set_all(&mut melted, &[("sekil", Some("kupoid")), ("cap", Some("7")), ("metaball", None)]);
        let m = edits(melted.apply(&w, &inp));
        assert!(!m.contains_key(&[3, 3, 3]));
        assert!(m.contains_key(&[0, 0, 0]));

        // The second action carves.
        let mut inp2 = inp.clone();
        inp2.secondary = true;
        assert!(edits(melted.apply(&w, &inp2)).values().all(|v| v == AIR));
    }

    #[test]
    fn path_lines_and_curves() {
        let w = W::new();
        let mut line = PathTool::default();
        line.set("tur", Some("bresenham")).unwrap();
        let o = edits(line.apply(&w, &input(vec![[0, 0, 0], [5, 0, 0]])));
        assert_eq!(o.len(), 6);

        let mut dda = PathTool::default();
        dda.set("tur", Some("dda")).unwrap();
        let o = edits(dda.apply(&w, &input(vec![[0, 0, 0], [4, 0, 0]])));
        assert!(o.contains_key(&[2, 1, 0]));
        assert!(!o.contains_key(&[2, 2, 0]));

        // Catmull-Rom reaches every point.
        let pts = vec![[0, 0, 0], [6, 3, 0], [12, 0, 4], [18, 5, 4]];
        let mut cat = PathTool::default();
        cat.set("yaricap", Some("0")).unwrap();
        let o = edits(cat.apply(&w, &input(pts.clone())));
        for p in &pts {
            assert!(o.contains_key(p), "{p:?}");
        }

        // Bezier keeps only the ends: the middle point is passed by.
        let mut bez = PathTool::default();
        set_all(&mut bez, &[("tur", Some("bezier")), ("yaricap", Some("0"))]);
        let o = edits(bez.apply(&w, &input(vec![[0, 0, 0], [5, 10, 0], [10, 0, 0]])));
        assert!(o.contains_key(&[0, 0, 0]) && o.contains_key(&[10, 0, 0]));
        assert!(o.contains_key(&[5, 5, 0]));
        assert!(!o.contains_key(&[5, 10, 0]));

        // A catenary sags below the line, or above it when inverted.
        let mut cate = PathTool::default();
        set_all(&mut cate, &[("tur", Some("katener")), ("yaricap", Some("0")), ("sarkma", Some("0.3"))]);
        let span = input(vec![[0, 0, 0], [10, 0, 0]]);
        assert!(edits(cate.apply(&w, &span)).contains_key(&[5, -3, 0]));
        cate.set("ters", None).unwrap();
        assert!(edits(cate.apply(&w, &span)).contains_key(&[5, 3, 0]));
    }

    #[test]
    fn path_loop_ground_and_keep() {
        let mut w = W::new();
        w.ground(6);
        let mut line = PathTool::default();
        set_all(&mut line, &[("tur", Some("bresenham")), ("dongu", None)]);
        let o = edits(line.apply(&w, &input(vec![[0, 5, 0], [4, 5, 0], [4, 5, 4]])));
        // The closing stretch from the last point back to the first.
        assert!(o.contains_key(&[2, 5, 2]));

        let mut g = PathTool::default();
        set_all(&mut g, &[("yaricap", Some("0")), ("zemin", None)]);
        let o = edits(g.apply(&w, &input(vec![[0, 3, 0]])));
        assert_eq!(o.keys().copied().collect::<Vec<Pos>>(), vec![[0, 1, 0], [0, 2, 0], [0, 3, 0]]);

        let mut keep = PathTool::default();
        set_all(&mut keep, &[("yaricap", Some("0")), ("koru", None)]);
        assert!(matches!(keep.apply(&w, &input(vec![[0, 0, 0]])), Outcome::Message(_)));
    }

    #[test]
    fn grid_blur_keeps_total_mass_in_the_middle() {
        let mut g = Grid::new([0, 0, 0], [9, 9, 9]).unwrap();
        for v in g.data.iter_mut() {
            *v = 1.0;
        }
        g.blur(1.5);
        assert!((g.get([4, 4, 4]) - 1.0).abs() < 1e-4);
        let mut h = Grid::new([0, 0, 0], [9, 9, 9]).unwrap();
        let i = h.index([4, 4, 4]);
        h.data[i] = 1.0;
        h.blur(1.0);
        let total: f32 = h.data.iter().sum();
        assert!((total - 1.0).abs() < 1e-3);
        assert!(h.get([4, 4, 4]) > h.get([5, 4, 4]));
    }
}
