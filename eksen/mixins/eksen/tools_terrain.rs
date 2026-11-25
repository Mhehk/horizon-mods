// Eksen — 16.10.1 (mihehk): the fluid, heightmap and manipulation tools.
// Written from scratch after the behaviour Axiom's documentation describes
// (axiomdocs.moulberry.com, "Fluid Tools", "Heightmap Tools", "Manipulation
// Tools"); no code, names or images of Axiom are used.
//
// Floodfill, Fluid Ball, Elevation, Slope, Smooth, Distort, Roughen, Shatter
// and Extrude. Each is a `Tool` over the `Blocks` trait: it reads blocks
// (`None` means not loaded, and such a block is never touched) and answers
// with the blocks to set. Every tool stops at `MAX_WORK` blocks.
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, VecDeque};

use super::eksen_core::*;

/// The most blocks any one tool reads or writes per use.
const MAX_WORK: usize = 1_000_000;
/// The largest brush radius an option accepts (a 65 block cube per point).
const MAX_RADIUS: i32 = 32;

const FACES: [Pos; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
const ORTH: [Pos; 4] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
const DIAG: [Pos; 4] = [[1, 0, 1], [1, 0, -1], [-1, 0, 1], [-1, 0, -1]];

fn add(a: Pos, d: Pos) -> Pos {
    [a[0] + d[0], a[1] + d[1], a[2] + d[2]]
}

fn mul(d: Pos, k: i32) -> Pos {
    [d[0] * k, d[1] * k, d[2] * k]
}

fn msg(text: &str) -> Outcome {
    Outcome::Message(vec![text.to_string()])
}

/// The edits, or a chat line when nothing would change.
fn finish(edits: BTreeMap<Pos, String>) -> Outcome {
    if edits.is_empty() {
        msg("Hiçbir blok değişmedi.")
    } else {
        Outcome::Edit(edits)
    }
}

fn unknown(option: &str) -> Result<(), String> {
    Err(format!("Bilinmeyen ayar: {option}"))
}

/// How far `d` is from the brush centre as 0 (centre) to 1 (edge), in the
/// metric of the brush's shape.
fn reach(shape: Shape, d: Pos, r: i32) -> f64 {
    let [x, y, z] = d.map(|v| v as f64);
    let e = r.max(0) as f64 + 0.5;
    let t = match shape {
        Shape::Sphere => (x * x + y * y + z * z).sqrt(),
        Shape::Cube => x.abs().max(y.abs()).max(z.abs()),
        Shape::Octahedron => x.abs() + y.abs() + z.abs(),
    };
    (t / e).clamp(0.0, 1.0)
}

/// The brush options every brush tool has.
#[derive(Clone, Copy, Debug)]
struct BrushOpt {
    radius: i32,
    shape: Shape,
}

impl BrushOpt {
    fn new(radius: i32) -> Self {
        BrushOpt { radius, shape: Shape::Sphere }
    }

    /// Handles `yaricap` and `sekil`; None if the option is not one of them.
    fn set(&mut self, option: &str, value: Option<&str>) -> Option<Result<(), String>> {
        match option {
            "yaricap" => Some(number::<i32>(value, 0, MAX_RADIUS).map(|v| self.radius = v)),
            "sekil" => Some(match value.and_then(Shape::parse) {
                Some(s) => {
                    self.shape = s;
                    Ok(())
                }
                None => Err("Şekil: küre, küp veya sekizyüzlü.".to_string()),
            }),
            _ => None,
        }
    }

    fn describe(&self, out: &mut Vec<String>) {
        out.push(format!("yaricap: {}", self.radius));
        out.push(format!("sekil: {}", self.shape.name()));
    }

    /// Every block the stroke's brushes cover, with how near the edge of its
    /// nearest brush it is (0 centre, 1 edge).
    fn weighted(&self, input: &Input) -> BTreeMap<Pos, f64> {
        let r = self.radius.max(0);
        let mut out: BTreeMap<Pos, f64> = BTreeMap::new();
        'points: for c in &input.stroke {
            for p in brush(*c, r, self.shape) {
                let t = reach(self.shape, [p[0] - c[0], p[1] - c[1], p[2] - c[2]], r);
                let e = out.entry(p).or_insert(t);
                if t < *e {
                    *e = t;
                }
                if out.len() >= MAX_WORK {
                    break 'points;
                }
            }
        }
        out
    }

    fn cells(&self, input: &Input) -> BTreeSet<Pos> {
        self.weighted(input).into_keys().collect()
    }
}

// ───────────────────────────── Floodfill ─────────────────────────────

/// Floodfill: fills the air around the hit block with the active block,
/// nearest first, up to a limit.
#[derive(Clone, Debug)]
pub struct Floodfill {
    /// The most blocks placed. Inferred: 1000.
    limit: usize,
    /// Only blocks at or above the starting layer.
    up: bool,
    /// Only blocks at or below the starting layer (on by default: it spreads
    /// "across and down").
    down: bool,
    /// Spreads sideways.
    horizontal: bool,
    /// With `horizontal`: also over corners.
    corners: bool,
}

impl Default for Floodfill {
    fn default() -> Self {
        Floodfill { limit: 1000, up: false, down: true, horizontal: true, corners: false }
    }
}

impl Tool for Floodfill {
    fn id(&self) -> &'static str {
        "taskin"
    }
    fn name(&self) -> &'static str {
        "Taşkın"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        match option {
            "limit" => self.limit = number::<usize>(value, 1, MAX_WORK)?,
            "yukari" => toggle(&mut self.up, value)?,
            "asagi" => toggle(&mut self.down, value)?,
            "yatay" => toggle(&mut self.horizontal, value)?,
            "koseler" => toggle(&mut self.corners, value)?,
            _ => return unknown(option),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        vec![
            format!("limit: {}", self.limit),
            format!("yukari: {}", on_off(self.up)),
            format!("asagi: {}", on_off(self.down)),
            format!("yatay: {}", on_off(self.horizontal)),
            format!("koseler: {}", on_off(self.corners)),
        ]
    }
    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        let Some(&hit) = input.stroke.first() else {
            return msg("Önce bir bloğa bakın.");
        };
        if input.active.is_empty() {
            return msg("Etkin blok seçili değil.");
        }
        let Some(hb) = world.block(hit) else {
            return msg("Blok yüklü değil.");
        };
        // From the air at the hit, or the air in front of the hit face.
        let start = if hb.air() {
            hit
        } else if input.face == [0, 0, 0] {
            return msg("Yüz bilinmiyor.");
        } else {
            add(hit, input.face)
        };
        if !world.block(start).is_some_and(|b| b.air()) {
            return msg("Başlangıç noktası dolu.");
        }
        let within = |p: Pos| (!self.up || p[1] >= start[1]) && (!self.down || p[1] <= start[1]);
        let mut steps: Vec<Pos> = vec![[0, 1, 0], [0, -1, 0]];
        if self.horizontal {
            steps.extend(ORTH);
            if self.corners {
                steps.extend(DIAG);
            }
        }
        let cap = self.limit.min(MAX_WORK);
        let mut seen: HashSet<Pos> = HashSet::new();
        seen.insert(start);
        let mut queue: VecDeque<Pos> = VecDeque::new();
        queue.push_back(start);
        let mut edits: BTreeMap<Pos, String> = BTreeMap::new();
        while let Some(p) = queue.pop_front() {
            if edits.len() >= cap {
                break;
            }
            edits.insert(p, input.active.clone());
            for d in &steps {
                let q = add(p, *d);
                if !within(q) || !seen.insert(q) {
                    continue;
                }
                if world.block(q).is_some_and(|b| b.air()) {
                    queue.push_back(q);
                }
            }
        }
        finish(edits)
    }
}

// ───────────────────────────── Fluid Ball ─────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sivi {
    Su,
    Lav,
    Kar,
}

impl Sivi {
    fn parse(w: &str) -> Option<Self> {
        Some(match w {
            "su" | "water" => Sivi::Su,
            "lav" | "lava" => Sivi::Lav,
            "kar" | "snow" => Sivi::Kar,
            _ => return None,
        })
    }
    fn name(self) -> &'static str {
        match self {
            Sivi::Su => "su",
            Sivi::Lav => "lav",
            Sivi::Kar => "kar",
        }
    }
    fn id(self) -> &'static str {
        match self {
            Sivi::Su => "minecraft:water",
            Sivi::Lav => "minecraft:lava",
            Sivi::Kar => "minecraft:snow",
        }
    }
    /// The block for a cell: `level` 0 is a source, `falling` a falling column.
    fn text(self, level: u8, falling: bool) -> String {
        match self {
            Sivi::Kar => {
                if level == 0 {
                    "minecraft:snow_block".to_string()
                } else {
                    format!("minecraft:snow[layers={}]", (8 - i32::from(level)).clamp(1, 8))
                }
            }
            _ => {
                if falling {
                    format!("{}[level=8]", self.id())
                } else if level == 0 {
                    self.id().to_string()
                } else {
                    format!("{}[level={}]", self.id(), level.min(7))
                }
            }
        }
    }
}

/// Fluid Ball: a brush of fluid sources that then flows over the terrain.
#[derive(Clone, Debug)]
pub struct FluidBall {
    b: BrushOpt,
    kind: Sivi,
    /// How far ahead a flow looks for a drop to run towards (1: not at all).
    /// Inferred: 4, as the game's own water does.
    quality: i32,
    /// How many blocks the fluid runs from a source. Inferred: 7.
    flow_length: i32,
    /// Removes flowing fluid of this kind in the brush that has no source.
    /// Inferred: off.
    remove_unsourced: bool,
    /// Flows over corners too. Inferred: off.
    fill_edges: bool,
    /// The whole brush becomes source blocks (otherwise only the top of each
    /// column does, and the rest falls). Inferred: off.
    infinite: bool,
    /// Flows sideways only over a solid block. Inferred: off.
    require_support: bool,
}

impl Default for FluidBall {
    fn default() -> Self {
        FluidBall {
            b: BrushOpt::new(3), // Inferred: 3
            kind: Sivi::Su,
            quality: 4,
            flow_length: 7,
            remove_unsourced: false,
            fill_edges: false,
            infinite: false,
            require_support: false,
        }
    }
}

impl Tool for FluidBall {
    fn id(&self) -> &'static str {
        "sivitop"
    }
    fn name(&self) -> &'static str {
        "Sıvı Topu"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        if let Some(r) = self.b.set(option, value) {
            return r;
        }
        match option {
            "sivi" => {
                self.kind = value.and_then(Sivi::parse).ok_or_else(|| "Sıvı: su, lav veya kar.".to_string())?
            }
            "kalite" => self.quality = number::<i32>(value, 1, 8)?,
            "akis_uzunlugu" => self.flow_length = number::<i32>(value, 1, 16)?,
            "kaynaksiz_sil" => toggle(&mut self.remove_unsourced, value)?,
            "kenar_doldur" => toggle(&mut self.fill_edges, value)?,
            "sonsuz_kaynak" => toggle(&mut self.infinite, value)?,
            "destek_gerekli" => toggle(&mut self.require_support, value)?,
            _ => return unknown(option),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        let mut v = Vec::new();
        self.b.describe(&mut v);
        v.push(format!("sivi: {}", self.kind.name()));
        v.push(format!("kalite: {}", self.quality));
        v.push(format!("akis_uzunlugu: {}", self.flow_length));
        v.push(format!("kaynaksiz_sil: {}", on_off(self.remove_unsourced)));
        v.push(format!("kenar_doldur: {}", on_off(self.fill_edges)));
        v.push(format!("sonsuz_kaynak: {}", on_off(self.infinite)));
        v.push(format!("destek_gerekli: {}", on_off(self.require_support)));
        v
    }
    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        if input.stroke.is_empty() {
            return msg("Önce bir bloğa bakın.");
        }
        let region = self.b.cells(input);
        // Cell: (flow distance, falling, depth of the fall).
        let mut cells: BTreeMap<Pos, (u8, bool, u16)> = BTreeMap::new();
        let mut heap: BinaryHeap<Reverse<(u8, Pos)>> = BinaryHeap::new();
        for &p in &region {
            if !world.block(p).is_some_and(|b| b.air()) {
                continue;
            }
            let top = !region.contains(&add(p, [0, 1, 0]));
            let falling = !(self.infinite || top);
            cells.insert(p, (0, falling, 0));
            heap.push(Reverse((0, p)));
        }
        if cells.is_empty() && !self.remove_unsourced {
            return msg("Sıvı için boş yer yok.");
        }
        let flow = self.flow_length as u8;
        while let Some(Reverse((lvl, p))) = heap.pop() {
            if cells.len() >= MAX_WORK {
                break;
            }
            let Some(&(clvl, falling, depth)) = cells.get(&p) else {
                continue;
            };
            if clvl != lvl {
                continue;
            }
            let below = add(p, [0, -1, 0]);
            let Some(bb) = world.block(below) else {
                continue;
            };
            let below_in = cells.contains_key(&below);
            // Over air it falls, and a fall restarts the flow where it lands.
            if bb.air() && !below_in {
                if self.kind != Sivi::Kar && depth < 64 {
                    cells.insert(below, (0, true, depth + 1));
                    heap.push(Reverse((0, below)));
                }
                continue;
            }
            if below_in && falling {
                continue;
            }
            if clvl >= flow {
                continue;
            }
            if (self.require_support || self.kind == Sivi::Kar) && !bb.solid {
                continue;
            }
            // Runs towards the nearest drop within `quality` blocks, if any.
            let mut best = usize::MAX;
            let mut pref: Vec<Pos> = Vec::new();
            if self.quality > 1 {
                for d in &ORTH {
                    let mut q = p;
                    for step in 1..=self.quality as usize {
                        q = add(q, *d);
                        let Some(b) = world.block(q) else {
                            break;
                        };
                        if !b.air() {
                            break;
                        }
                        if world.block(add(q, [0, -1, 0])).is_some_and(|x| x.air()) {
                            if step < best {
                                best = step;
                                pref.clear();
                            }
                            if step == best {
                                pref.push(*d);
                            }
                            break;
                        }
                    }
                }
            }
            let mut dirs: Vec<Pos> = pref;
            if dirs.is_empty() {
                dirs.extend(ORTH);
                if self.fill_edges {
                    dirs.extend(DIAG);
                }
            }
            for d in dirs {
                let q = add(p, d);
                if cells.contains_key(&q) {
                    continue;
                }
                if world.block(q).is_some_and(|b| b.air()) {
                    cells.insert(q, (clvl + 1, false, 0));
                    heap.push(Reverse((clvl + 1, q)));
                }
            }
        }
        let mut edits: BTreeMap<Pos, String> = BTreeMap::new();
        for (p, (lvl, falling, _)) in &cells {
            edits.insert(*p, self.kind.text(*lvl, *falling));
        }
        if self.remove_unsourced && self.kind != Sivi::Kar {
            for &p in &region {
                if cells.contains_key(&p) {
                    continue;
                }
                if let Some(b) = world.block(p) {
                    if b.id == self.kind.id() && !b.state.is_empty() && b.state != "level=0" {
                        edits.insert(p, AIR.to_string());
                    }
                }
            }
        }
        finish(edits)
    }
}

// ─────────────────────── Heightmap helpers (columns) ───────────────────────

/// The top solid block of a column and the blocks to build with.
struct Surface {
    y: i32,
    /// The top block as a command writes it.
    top: String,
    /// What lies under it (the top again if that is not solid).
    sub: String,
}

/// The topmost solid block of column (x, z) within `lo..=hi`. Blocks that are
/// not loaded are passed over.
fn surface(world: &dyn Blocks, x: i32, z: i32, lo: i32, hi: i32) -> Option<Surface> {
    let mut y = hi;
    while y >= lo {
        if let Some(b) = world.block([x, y, z]) {
            if b.solid {
                let top = block_string(&b);
                let sub = match world.block([x, y - 1, z]) {
                    Some(s) if s.solid => block_string(&s),
                    _ => top.clone(),
                };
                return Some(Surface { y, top, sub });
            }
        }
        y -= 1;
    }
    None
}

/// Moves a column's surface by `delta` blocks: raising builds up from it,
/// lowering digs down and leaves the top material on the new surface.
fn reshape(world: &dyn Blocks, edits: &mut BTreeMap<Pos, String>, x: i32, z: i32, s: &Surface, delta: i32) {
    if delta > 0 {
        let mut placed = 0;
        for k in 1..=delta {
            if !matches!(world.block([x, s.y + k, z]), Some(b) if !b.solid) {
                break;
            }
            placed = k;
        }
        if placed == 0 {
            return;
        }
        for y in s.y..s.y + placed {
            edits.insert([x, y, z], s.sub.clone());
        }
        edits.insert([x, s.y + placed, z], s.top.clone());
    } else if delta < 0 {
        let mut n = -delta;
        for k in 1..n {
            if world.block([x, s.y - k, z]).is_none() {
                n = k;
                break;
            }
        }
        for y in (s.y - n + 1)..=s.y {
            edits.insert([x, y, z], AIR.to_string());
        }
        let p = [x, s.y - n, z];
        if world.block(p).is_some_and(|b| b.solid) {
            edits.insert(p, s.top.clone());
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Falloff {
    Yok,
    Dogrusal,
    Yumusak,
}

impl Falloff {
    fn parse(w: &str) -> Option<Self> {
        Some(match w {
            "yok" | "none" => Falloff::Yok,
            "dogrusal" | "doğrusal" | "linear" => Falloff::Dogrusal,
            "yumusak" | "yumuşak" | "smooth" => Falloff::Yumusak,
            _ => return None,
        })
    }
    fn name(self) -> &'static str {
        match self {
            Falloff::Yok => "yok",
            Falloff::Dogrusal => "dogrusal",
            Falloff::Yumusak => "yumusak",
        }
    }
    /// The strength at `t` (0 centre, 1 edge).
    fn weight(self, t: f64) -> f64 {
        match self {
            Falloff::Yok => 1.0,
            Falloff::Dogrusal => 1.0 - t,
            Falloff::Yumusak => 1.0 - t * t * (3.0 - 2.0 * t),
        }
    }
}

/// The columns a stroke covers: (x, z) to (strength, y of the stroke point
/// that first reached it). `continuous` adds the strengths of overlapping
/// brushes; otherwise the strongest wins.
fn columns(b: &BrushOpt, input: &Input, continuous: bool, falloff: Falloff) -> BTreeMap<(i32, i32), (f64, i32)> {
    let r = b.radius.max(0);
    let mut cols: BTreeMap<(i32, i32), (f64, i32)> = BTreeMap::new();
    for c in &input.stroke {
        for dx in -r..=r {
            for dz in -r..=r {
                if !b.shape.contains([dx, 0, dz], r) {
                    continue;
                }
                let w = falloff.weight(reach(b.shape, [dx, 0, dz], r));
                let e = cols.entry((c[0] + dx, c[2] + dz)).or_insert((0.0, c[1]));
                if continuous {
                    e.0 += w;
                } else if w > e.0 {
                    e.0 = w;
                }
            }
        }
        if cols.len() >= MAX_WORK {
            break;
        }
    }
    cols
}

// ───────────────────────────── Elevation ─────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ElevMode {
    Yukselt,
    Alcalt,
    Duzle,
}

/// Elevation: raises, lowers or flattens the terrain columns under the brush.
/// `Input.secondary` swaps raise and lower.
#[derive(Clone, Debug)]
pub struct Elevation {
    b: BrushOpt,
    /// Half the height of the cylinder that picks the surface: the surface is
    /// looked for this far above and below the stroke point. Inferred: 16.
    y_limit: i32,
    mode: ElevMode,
    /// Passes of blur over the height change (0: none). Inferred: 0.
    smoothing: i32,
    /// Each stroke point applies its own change on top of the others
    /// (otherwise one change per column). Inferred: off.
    continuous: bool,
    /// Blocks per application. Inferred: 2.
    amount: i32,
    /// Inferred: smooth.
    falloff: Falloff,
}

impl Default for Elevation {
    fn default() -> Self {
        Elevation {
            b: BrushOpt::new(5), // Inferred: 5
            y_limit: 16,
            mode: ElevMode::Yukselt,
            smoothing: 0,
            continuous: false,
            amount: 2,
            falloff: Falloff::Yumusak,
        }
    }
}

impl Tool for Elevation {
    fn id(&self) -> &'static str {
        "yukselti"
    }
    fn name(&self) -> &'static str {
        "Yükselti"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        if let Some(r) = self.b.set(option, value) {
            return r;
        }
        match option {
            "y_siniri" => self.y_limit = number::<i32>(value, 1, 128)?,
            "mod" => {
                self.mode = match value {
                    Some("yukselt") => ElevMode::Yukselt,
                    Some("alcalt") => ElevMode::Alcalt,
                    Some("duzle") => ElevMode::Duzle,
                    _ => return Err("Mod: yukselt, alcalt veya duzle.".to_string()),
                }
            }
            "yumusatma" => self.smoothing = number::<i32>(value, 0, 8)?,
            "surekli" => toggle(&mut self.continuous, value)?,
            "miktar" => self.amount = number::<i32>(value, 1, 64)?,
            "dusus" => {
                self.falloff = value
                    .and_then(Falloff::parse)
                    .ok_or_else(|| "Düşüş: yok, dogrusal veya yumusak.".to_string())?
            }
            _ => return unknown(option),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        let mut v = Vec::new();
        self.b.describe(&mut v);
        v.push(format!("y_siniri: {}", self.y_limit));
        v.push(format!(
            "mod: {}",
            match self.mode {
                ElevMode::Yukselt => "yukselt",
                ElevMode::Alcalt => "alcalt",
                ElevMode::Duzle => "duzle",
            }
        ));
        v.push(format!("yumusatma: {}", self.smoothing));
        v.push(format!("surekli: {}", on_off(self.continuous)));
        v.push(format!("miktar: {}", self.amount));
        v.push(format!("dusus: {}", self.falloff.name()));
        v
    }
    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        let Some(&c0) = input.stroke.first() else {
            return msg("Önce bir bloğa bakın.");
        };
        let cols = columns(&self.b, input, self.continuous, self.falloff);
        let yl = self.y_limit;
        let mut surfaces: BTreeMap<(i32, i32), Surface> = BTreeMap::new();
        let mut delta: BTreeMap<(i32, i32), f64> = BTreeMap::new();
        for (&(x, z), &(v, by)) in &cols {
            let Some(s) = surface(world, x, z, by - yl, by + yl) else {
                continue;
            };
            let d = match self.mode {
                ElevMode::Duzle => (c0[1] - s.y) as f64 * v.min(1.0),
                m => {
                    let up = (m == ElevMode::Yukselt) != input.secondary;
                    let sign = if up { 1.0 } else { -1.0 };
                    sign * f64::from(self.amount) * v
                }
            };
            delta.insert((x, z), d);
            surfaces.insert((x, z), s);
        }
        for _ in 0..self.smoothing {
            let mut next: BTreeMap<(i32, i32), f64> = BTreeMap::new();
            for (&(x, z), _) in &delta {
                let mut sum = 0.0;
                let mut n = 0.0;
                for dx in -1..=1 {
                    for dz in -1..=1 {
                        if let Some(v) = delta.get(&(x + dx, z + dz)) {
                            sum += *v;
                            n += 1.0;
                        }
                    }
                }
                next.insert((x, z), sum / n);
            }
            delta = next;
        }
        let mut edits: BTreeMap<Pos, String> = BTreeMap::new();
        for (&(x, z), &d) in &delta {
            let di = (d.round() as i32).clamp(-256, 256);
            if di != 0 {
                reshape(world, &mut edits, x, z, &surfaces[&(x, z)], di);
            }
        }
        finish(edits)
    }
}

// ───────────────────────────── Slope ─────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlopeMode {
    Duzlem,
    Koni,
}

/// Slope: levels the terrain along a ramp between a target point and the
/// point the stroke starts from (or, with no target set, between the first and
/// last point of the stroke).
#[derive(Clone, Debug)]
pub struct Slope {
    b: BrushOpt,
    /// Half the height of the cylinder that picks the surface. Inferred: 16.
    y_limit: i32,
    mode: SlopeMode,
    /// 1: the terrain is cut to the slope exactly; less: the change fades out
    /// the further the terrain is from the slope. Inferred: 1.
    sharpness: f64,
    /// The target point (`/eksen ayar hedef x y z`); None: the stroke's first
    /// point is the target.
    target: Option<Pos>,
}

impl Default for Slope {
    fn default() -> Self {
        Slope { b: BrushOpt::new(4), y_limit: 16, mode: SlopeMode::Duzlem, sharpness: 1.0, target: None }
    }
}

impl Tool for Slope {
    fn id(&self) -> &'static str {
        "egim"
    }
    fn name(&self) -> &'static str {
        "Eğim"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        if let Some(r) = self.b.set(option, value) {
            return r;
        }
        match option {
            "y_siniri" => self.y_limit = number::<i32>(value, 1, 128)?,
            "mod" => {
                self.mode = match value {
                    Some("duzlem") => SlopeMode::Duzlem,
                    Some("koni") => SlopeMode::Koni,
                    _ => return Err("Mod: duzlem veya koni.".to_string()),
                }
            }
            "keskinlik" => self.sharpness = number::<f64>(value, 0.0, 1.0)?,
            "hedef" => {
                self.target = match value {
                    None => None,
                    Some(v) => {
                        let words: Vec<&str> = v.split_whitespace().collect();
                        if words.len() != 3 {
                            return Err("hedef: x y z".to_string());
                        }
                        let mut p = [0i32; 3];
                        for (i, w) in words.iter().enumerate() {
                            p[i] = w.parse().map_err(|_| format!("Geçersiz sayı: {w}"))?;
                        }
                        Some(p)
                    }
                }
            }
            _ => return unknown(option),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        let mut v = Vec::new();
        self.b.describe(&mut v);
        v.push(format!("y_siniri: {}", self.y_limit));
        v.push(format!("mod: {}", if self.mode == SlopeMode::Duzlem { "duzlem" } else { "koni" }));
        v.push(format!("keskinlik: {}", self.sharpness));
        v.push(match self.target {
            Some(t) => format!("hedef: {} {} {}", t[0], t[1], t[2]),
            None => "hedef: yok (vuruşun ilk noktası)".to_string(),
        });
        v
    }
    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        let (t, b) = match (self.target, input.stroke.as_slice()) {
            (Some(t), [first, ..]) => (t, *first),
            (None, [first, .., last]) => (*first, *last),
            _ => return msg("Eğim için iki nokta gerekli."),
        };
        let dx = f64::from(b[0] - t[0]);
        let dz = f64::from(b[2] - t[2]);
        let hdist = (dx * dx + dz * dz).sqrt();
        if hdist < 0.5 {
            return msg("Eğim için iki farklı nokta gerekli.");
        }
        let k = f64::from(b[1] - t[1]) / hdist;
        let (dirx, dirz) = (dx / hdist, dz / hdist);
        let yl = self.y_limit;
        let cols = columns(&self.b, input, false, Falloff::Yok);
        let mut edits: BTreeMap<Pos, String> = BTreeMap::new();
        for (&(x, z), &(_, by)) in &cols {
            let fx = f64::from(x - t[0]);
            let fz = f64::from(z - t[2]);
            let h = match self.mode {
                SlopeMode::Duzlem => f64::from(t[1]) + k * (fx * dirx + fz * dirz),
                SlopeMode::Koni => f64::from(t[1]) + k * (fx * fx + fz * fz).sqrt(),
            };
            let Some(s) = surface(world, x, z, by - yl, by + yl) else {
                continue;
            };
            let gap = h.round() as i32 - s.y;
            let far = (f64::from(gap.abs()) / f64::from(yl.max(1))).min(1.0);
            let w = 1.0 - (1.0 - self.sharpness) * far;
            let d = (f64::from(gap) * w).round() as i32;
            reshape(world, &mut edits, x, z, &s, d.clamp(-256, 256));
        }
        finish(edits)
    }
}

// ───────────────────────────── Smooth ─────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SmoothMode {
    Kararli,
    Erit,
    Buyut,
}

/// Smooth: a Gaussian blur of the solid blocks that keeps the block count
/// (times the ratio) by filling the best-supported spots first.
#[derive(Clone, Debug)]
pub struct Smooth {
    b: BrushOpt,
    /// The blur's reach in blocks (default 2).
    strength: i32,
    /// The block count kept, in percent (default 100).
    ratio: i32,
    mode: SmoothMode,
    /// Blends the brush's edge into what lies around it (default on).
    fix_edges: bool,
}

impl Default for Smooth {
    fn default() -> Self {
        Smooth { b: BrushOpt::new(5), strength: 2, ratio: 100, mode: SmoothMode::Kararli, fix_edges: true }
    }
}

/// Whether the block at `p` is solid, from `cache` or the world.
fn solid_cached(world: &dyn Blocks, cache: &mut HashMap<Pos, Option<bool>>, p: Pos) -> Option<bool> {
    *cache.entry(p).or_insert_with(|| world.block(p).map(|b| b.solid))
}

impl Tool for Smooth {
    fn id(&self) -> &'static str {
        "yumusat"
    }
    fn name(&self) -> &'static str {
        "Yumuşat"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        if let Some(r) = self.b.set(option, value) {
            return r;
        }
        match option {
            "guc" => self.strength = number::<i32>(value, 1, 10)?,
            "oran" => self.ratio = number::<i32>(value, 50, 150)?,
            "mod" => {
                self.mode = match value {
                    Some("kararli") => SmoothMode::Kararli,
                    Some("erit") => SmoothMode::Erit,
                    Some("buyut") => SmoothMode::Buyut,
                    _ => return Err("Mod: kararli, erit veya buyut.".to_string()),
                }
            }
            "kenar_duzelt" => toggle(&mut self.fix_edges, value)?,
            _ => return unknown(option),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        let mut v = Vec::new();
        self.b.describe(&mut v);
        v.push(format!("guc: {}", self.strength));
        v.push(format!("oran: %{}", self.ratio));
        v.push(format!(
            "mod: {}",
            match self.mode {
                SmoothMode::Kararli => "kararli",
                SmoothMode::Erit => "erit",
                SmoothMode::Buyut => "buyut",
            }
        ));
        v.push(format!("kenar_duzelt: {}", on_off(self.fix_edges)));
        v
    }
    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        if input.stroke.is_empty() {
            return msg("Önce bir bloğa bakın.");
        }
        let region = self.b.cells(input);
        let k = self.strength;
        let sigma = (f64::from(k) * 0.5).max(0.5);
        let mut kern: Vec<(Pos, f64)> = Vec::new();
        for dx in -k..=k {
            for dy in -k..=k {
                for dz in -k..=k {
                    let d2 = dx * dx + dy * dy + dz * dz;
                    if d2 > k * k + k {
                        continue;
                    }
                    kern.push(([dx, dy, dz], (-f64::from(d2) / (2.0 * sigma * sigma)).exp()));
                }
            }
        }
        // Nearest first: the first entry is the block itself.
        kern.sort_by_key(|(o, _)| o[0] * o[0] + o[1] * o[1] + o[2] * o[2]);
        let budget = (MAX_WORK / kern.len().max(1)).max(1);
        let mut cache: HashMap<Pos, Option<bool>> = HashMap::new();
        let mut cells: Vec<Pos> = Vec::new();
        for &p in &region {
            if cells.len() >= budget {
                break;
            }
            if solid_cached(world, &mut cache, p).is_some() {
                cells.push(p);
            }
        }
        let mut vals: Vec<f64> = Vec::with_capacity(cells.len());
        let mut origs: Vec<bool> = Vec::with_capacity(cells.len());
        let mut solid_n = 0usize;
        for &p in &cells {
            let orig = solid_cached(world, &mut cache, p) == Some(true);
            if orig {
                solid_n += 1;
            }
            let mut sum = 0.0;
            let mut tot = 0.0;
            for (o, w) in &kern {
                if let Some(s) = solid_cached(world, &mut cache, add(p, *o)) {
                    tot += *w;
                    if s {
                        sum += *w;
                    }
                }
            }
            let mut v = if tot > 0.0 { sum / tot } else { 0.0 };
            if self.fix_edges && FACES.iter().any(|d| !region.contains(&add(p, *d))) {
                v = 0.5 * v + 0.5 * if orig { 1.0 } else { 0.0 };
            }
            vals.push(v);
            origs.push(orig);
        }
        // The mass to keep: the solid count times the ratio (melt and grow
        // bias it down and up).
        let bias = match self.mode {
            SmoothMode::Kararli => 1.0,
            SmoothMode::Erit => 0.95, // Inferred
            SmoothMode::Buyut => 1.05, // Inferred
        };
        let n = ((solid_n as f64) * f64::from(self.ratio) / 100.0 * bias).round() as usize;
        let n = n.min(cells.len());
        let mut order: Vec<usize> = (0..cells.len()).collect();
        order.sort_by(|&a, &b| vals[b].total_cmp(&vals[a]).then(cells[a].cmp(&cells[b])));
        let mut want = vec![false; cells.len()];
        for &i in order.iter().take(n) {
            want[i] = true;
        }
        let mut edits: BTreeMap<Pos, String> = BTreeMap::new();
        for (i, &p) in cells.iter().enumerate() {
            if want[i] && !origs[i] {
                // Becomes solid: of the nearest solid block's kind.
                for (o, _) in kern.iter().skip(1) {
                    if let Some(b) = world.block(add(p, *o)) {
                        if b.solid {
                            edits.insert(p, block_string(&b));
                            break;
                        }
                    }
                }
            } else if !want[i] && origs[i] {
                edits.insert(p, AIR.to_string());
            }
        }
        finish(edits)
    }
}

// ───────────────────────────── Distort ─────────────────────────────

/// Distort: domain distortion with a noise field, so terrain gets bumpy.
#[derive(Clone, Debug)]
pub struct Distort {
    b: BrushOpt,
    /// Blocks per noise feature. Inferred: 8.
    scale: f64,
    seed: u32,
    /// How far blocks are displaced (the docs' example: 2).
    distance: f64,
    /// Own distances per axis.
    separate_axis: bool,
    distance_axis: [f64; 3],
    /// Fades the distortion out towards the brush's edge (default on).
    smooth_edges: bool,
}

impl Default for Distort {
    fn default() -> Self {
        Distort {
            b: BrushOpt::new(5), // Inferred: 5
            scale: 8.0,
            seed: 0,
            distance: 2.0,
            separate_axis: false,
            distance_axis: [2.0; 3],
            smooth_edges: true,
        }
    }
}

/// Whether two blocks are the same as far as an edit goes (all airs alike).
fn same_block(a: &Block, b: &Block) -> bool {
    (a.air() && b.air()) || block_string(a) == block_string(b)
}

impl Tool for Distort {
    fn id(&self) -> &'static str {
        "bozma"
    }
    fn name(&self) -> &'static str {
        "Bozma"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        if let Some(r) = self.b.set(option, value) {
            return r;
        }
        match option {
            "olcek" => self.scale = number::<f64>(value, 0.5, 256.0)?,
            "tohum" => self.seed = number::<u32>(value, 0, u32::MAX)?,
            "mesafe" => self.distance = number::<f64>(value, 0.0, 32.0)?,
            "mesafe_x" => self.distance_axis[0] = number::<f64>(value, 0.0, 32.0)?,
            "mesafe_y" => self.distance_axis[1] = number::<f64>(value, 0.0, 32.0)?,
            "mesafe_z" => self.distance_axis[2] = number::<f64>(value, 0.0, 32.0)?,
            "ayri_eksen" => toggle(&mut self.separate_axis, value)?,
            "kenar_yumusat" => toggle(&mut self.smooth_edges, value)?,
            _ => return unknown(option),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        let mut v = Vec::new();
        self.b.describe(&mut v);
        v.push(format!("olcek: {}", self.scale));
        v.push(format!("tohum: {}", self.seed));
        v.push(format!("mesafe: {}", self.distance));
        v.push(format!("ayri_eksen: {}", on_off(self.separate_axis)));
        if self.separate_axis {
            v.push(format!(
                "mesafe_x/y/z: {} {} {}",
                self.distance_axis[0], self.distance_axis[1], self.distance_axis[2]
            ));
        }
        v.push(format!("kenar_yumusat: {}", on_off(self.smooth_edges)));
        v
    }
    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        if input.stroke.is_empty() {
            return msg("Önce bir bloğa bakın.");
        }
        let dist = if self.separate_axis { self.distance_axis } else { [self.distance; 3] };
        let mut edits: BTreeMap<Pos, String> = BTreeMap::new();
        for (p, t) in self.b.weighted(input) {
            let w = if self.smooth_edges { ((1.0 - t) / 0.4).clamp(0.0, 1.0) } else { 1.0 };
            let at = p.map(|v| v as f64);
            let mut q = p;
            for a in 0..3 {
                // Inferred: the noise's typical size is well under 1, so it is
                // doubled to make `distance` the typical displacement.
                let n = noise3(at, self.scale, u64::from(self.seed).wrapping_add(a as u64));
                q[a] = (at[a] + n * dist[a] * 2.0 * w).round() as i32;
            }
            if q == p {
                continue;
            }
            let (Some(here), Some(src)) = (world.block(p), world.block(q)) else {
                continue;
            };
            if !same_block(&here, &src) {
                edits.insert(p, if src.air() { AIR.to_string() } else { block_string(&src) });
            }
        }
        finish(edits)
    }
}

// ───────────────────────────── Roughen ─────────────────────────────

/// Roughen: makes the surface rugged by digging into flat parts and building
/// onto others, at random.
#[derive(Clone, Debug)]
pub struct Roughen {
    b: BrushOpt,
    /// The most faces a block may have exposed (or touching) to be changed
    /// (1 to 4). Inferred: 2.
    faces: i32,
    /// The share of the candidates changed (0 to 1). Inferred: 0.5.
    ratio: f64,
    seed: u32,
}

impl Default for Roughen {
    fn default() -> Self {
        Roughen { b: BrushOpt::new(5), faces: 2, ratio: 0.5, seed: 0 }
    }
}

impl Tool for Roughen {
    fn id(&self) -> &'static str {
        "puruz"
    }
    fn name(&self) -> &'static str {
        "Pürüz"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        if let Some(r) = self.b.set(option, value) {
            return r;
        }
        match option {
            "yuz" => self.faces = number::<i32>(value, 1, 4)?,
            "oran" => self.ratio = number::<f64>(value, 0.0, 1.0)?,
            "tohum" => self.seed = number::<u32>(value, 0, u32::MAX)?,
            _ => return unknown(option),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        let mut v = Vec::new();
        self.b.describe(&mut v);
        v.push(format!("yuz: {}", self.faces));
        v.push(format!("oran: {}", self.ratio));
        v.push(format!("tohum: {}", self.seed));
        v
    }
    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        let Some(first) = input.stroke.first() else {
            return msg("Önce bir bloğa bakın.");
        };
        let region = self.b.cells(input);
        let mix = (first[0] as i64 as u64).wrapping_mul(31)
            ^ (first[1] as i64 as u64).wrapping_mul(131)
            ^ (first[2] as i64 as u64);
        let mut rng = Rng::new(u64::from(self.seed) ^ mix);
        let faces = self.faces as usize;
        let mut edits: BTreeMap<Pos, String> = BTreeMap::new();
        for &p in &region {
            let Some(b) = world.block(p) else {
                continue;
            };
            if b.solid {
                // A surface block: digs it away.
                let exposed = FACES
                    .iter()
                    .filter(|d| world.block(add(p, **d)).is_some_and(|n| !n.solid))
                    .count();
                if (1..=faces).contains(&exposed) && rng.next_f64() < self.ratio {
                    edits.insert(p, AIR.to_string());
                }
            } else if b.air() {
                // Air against the surface: builds onto it.
                let mut touching = 0usize;
                let mut src: Option<String> = None;
                for d in &FACES {
                    if let Some(nb) = world.block(add(p, *d)) {
                        if nb.solid {
                            touching += 1;
                            if src.is_none() {
                                src = Some(block_string(&nb));
                            }
                        }
                    }
                }
                if (1..=faces).contains(&touching) && rng.next_f64() < self.ratio {
                    if let Some(s) = src {
                        edits.insert(p, s);
                    }
                }
            }
        }
        finish(edits)
    }
}

// ───────────────────────────── Shatter ─────────────────────────────

/// Which noise Shatter cracks by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Axes {
    /// One 3D Voronoi noise.
    Full,
    /// A 1D noise per chosen axis: cracks are planes across that axis.
    Lines([bool; 3]),
}

/// The feature point of lattice cell `c`, each part in `0..1`.
fn cell_point(seed: u64, c: [i64; 3]) -> [f64; 3] {
    let mut h = seed ^ 0xA076_1D64_78BD_642F;
    for v in c {
        h = (h ^ v as u64).wrapping_mul(0x1000_0000_01B3);
        h ^= h >> 29;
    }
    let mut r = Rng::new(h);
    r.next_u64();
    [r.next_f64(), r.next_f64(), r.next_f64()]
}

/// Keeps the two smallest of the distances seen.
fn keep2(d1: &mut f64, d2: &mut f64, d: f64) {
    if d < *d1 {
        *d2 = *d1;
        *d1 = d;
    } else if d < *d2 {
        *d2 = d;
    }
}

/// How much farther (in blocks) the second nearest feature point of a 1D
/// Voronoi noise is than the nearest: near 0 on a cell edge.
fn gap1(x: f64, scale: f64, seed: u64) -> f64 {
    let u = x / scale;
    let c = u.floor() as i64;
    let (mut d1, mut d2) = (f64::MAX, f64::MAX);
    for dc in -1..=1i64 {
        let cc = c + dc;
        let fp = cc as f64 + cell_point(seed, [cc, 0, 0])[0];
        keep2(&mut d1, &mut d2, (u - fp).abs());
    }
    (d2 - d1) * scale
}

/// The same for a 3D noise.
fn gap3(p: [f64; 3], scale: f64, seed: u64) -> f64 {
    let u = p.map(|v| v / scale);
    let c = u.map(|v| v.floor() as i64);
    let (mut d1, mut d2) = (f64::MAX, f64::MAX);
    for dx in -1..=1i64 {
        for dy in -1..=1i64 {
            for dz in -1..=1i64 {
                let cc = [c[0] + dx, c[1] + dy, c[2] + dz];
                let f = cell_point(seed, cc);
                let d = (0..3).map(|a| (u[a] - (cc[a] as f64 + f[a])).powi(2)).sum::<f64>().sqrt();
                keep2(&mut d1, &mut d2, d);
            }
        }
    }
    (d2 - d1) * scale
}

/// Shatter: cracks the terrain along the edges of a Voronoi noise.
#[derive(Clone, Debug)]
pub struct Shatter {
    b: BrushOpt,
    /// Blocks per noise cell. Inferred: 8.
    scale: f64,
    /// How wide the cracks are, in blocks. Inferred: 1.
    width: f64,
    /// Fills the cracks with the active block instead of air.
    use_active: bool,
    axes: Axes,
    seed: u32,
}

impl Default for Shatter {
    fn default() -> Self {
        Shatter { b: BrushOpt::new(5), scale: 8.0, width: 1.0, use_active: false, axes: Axes::Full, seed: 0 }
    }
}

impl Shatter {
    fn crack(&self, p: Pos) -> bool {
        let f = p.map(|v| v as f64);
        let seed = u64::from(self.seed);
        match self.axes {
            Axes::Full => gap3(f, self.scale, seed) < self.width,
            Axes::Lines(l) => (0..3).any(|a| l[a] && gap1(f[a], self.scale, seed.wrapping_add(a as u64 + 1)) < self.width),
        }
    }
}

impl Tool for Shatter {
    fn id(&self) -> &'static str {
        "parcala"
    }
    fn name(&self) -> &'static str {
        "Parçala"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        if let Some(r) = self.b.set(option, value) {
            return r;
        }
        match option {
            "olcek" => self.scale = number::<f64>(value, 0.5, 256.0)?,
            "genislik" => self.width = number::<f64>(value, 0.0, 32.0)?,
            "etkin_blok" => toggle(&mut self.use_active, value)?,
            "tohum" => self.seed = number::<u32>(value, 0, u32::MAX)?,
            "eksen" => {
                let w = value.unwrap_or("");
                self.axes = if w == "3b" {
                    Axes::Full
                } else if !w.is_empty() && w.chars().all(|c| matches!(c, 'x' | 'y' | 'z')) {
                    Axes::Lines([w.contains('x'), w.contains('y'), w.contains('z')])
                } else {
                    return Err("Eksen: 3b veya x, y, z harfleri (ör. xz).".to_string());
                }
            }
            _ => return unknown(option),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        let mut v = Vec::new();
        self.b.describe(&mut v);
        v.push(format!("olcek: {}", self.scale));
        v.push(format!("genislik: {}", self.width));
        v.push(format!("etkin_blok: {}", on_off(self.use_active)));
        v.push(match self.axes {
            Axes::Full => "eksen: 3b".to_string(),
            Axes::Lines(l) => {
                let mut s = String::new();
                for (i, c) in ['x', 'y', 'z'].iter().enumerate() {
                    if l[i] {
                        s.push(*c);
                    }
                }
                format!("eksen: {s}")
            }
        });
        v.push(format!("tohum: {}", self.seed));
        v
    }
    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        if input.stroke.is_empty() {
            return msg("Önce bir bloğa bakın.");
        }
        if self.use_active && input.active.is_empty() {
            return msg("Etkin blok seçili değil.");
        }
        let fill = if self.use_active { input.active.clone() } else { AIR.to_string() };
        let mut edits: BTreeMap<Pos, String> = BTreeMap::new();
        for p in self.b.cells(input) {
            if world.block(p).is_some_and(|b| b.solid) && self.crack(p) {
                edits.insert(p, fill.clone());
            }
        }
        finish(edits)
    }
}

// ───────────────────────────── Extrude ─────────────────────────────

/// Which blocks Extrude takes as part of the surface (like Magic Select).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Karsilastir {
    Blok,
    Durum,
    Kati,
    Herhangi,
}

impl Karsilastir {
    fn parse(w: &str) -> Option<Self> {
        Some(match w {
            "blok" | "block" => Karsilastir::Blok,
            "durum" | "state" => Karsilastir::Durum,
            "kati" | "katı" | "solid" => Karsilastir::Kati,
            "herhangi" | "any" => Karsilastir::Herhangi,
            _ => return None,
        })
    }
    fn name(self) -> &'static str {
        match self {
            Karsilastir::Blok => "blok",
            Karsilastir::Durum => "durum",
            Karsilastir::Kati => "kati",
            Karsilastir::Herhangi => "herhangi",
        }
    }
    fn matches(self, seed: &Block, b: &Block) -> bool {
        match self {
            Karsilastir::Blok => seed.id == b.id,
            Karsilastir::Durum => seed.id == b.id && seed.state == b.state,
            Karsilastir::Kati => b.solid,
            Karsilastir::Herhangi => !b.air(),
        }
    }
}

/// Extrude: grows the surface that was hit outwards along its face, or (with
/// `Input.secondary`) pulls it in.
#[derive(Clone, Debug)]
pub struct Extrude {
    /// The most blocks of surface taken (default 100 000).
    limit: usize,
    compare: Karsilastir,
    /// Layers per use. Inferred: 1.
    count: i32,
    /// A surface one block thick moves instead of being copied. Inferred: off.
    displace: bool,
}

impl Default for Extrude {
    fn default() -> Self {
        Extrude { limit: 100_000, compare: Karsilastir::Blok, count: 1, displace: false }
    }
}

impl Tool for Extrude {
    fn id(&self) -> &'static str {
        "cikinti"
    }
    fn name(&self) -> &'static str {
        "Çıkıntı"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        match option {
            "limit" => self.limit = number::<usize>(value, 1, MAX_WORK)?,
            "karsilastir" => {
                self.compare = value
                    .and_then(Karsilastir::parse)
                    .ok_or_else(|| "Karşılaştır: blok, durum, kati veya herhangi.".to_string())?
            }
            "adet" => self.count = number::<i32>(value, 1, 64)?,
            "yer_degistir" => toggle(&mut self.displace, value)?,
            _ => return unknown(option),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        vec![
            format!("limit: {}", self.limit),
            format!("karsilastir: {}", self.compare.name()),
            format!("adet: {}", self.count),
            format!("yer_degistir: {}", on_off(self.displace)),
        ]
    }
    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        let Some(&hit) = input.stroke.first() else {
            return msg("Önce bir bloğa bakın.");
        };
        let f = input.face;
        let Some(axis) = (0..3).find(|&i| f[i] != 0) else {
            return msg("Yüz bilinmiyor.");
        };
        if f.iter().filter(|v| **v != 0).count() != 1 {
            return msg("Yüz bilinmiyor.");
        }
        let Some(seed) = world.block(hit).filter(|b| !b.air()) else {
            return msg("Bir blok seçin.");
        };
        let expose = |p: Pos| world.block(add(p, f)).is_some_and(|b| !b.solid);
        let ok = |p: Pos| world.block(p).is_some_and(|b| self.compare.matches(&seed, &b)) && expose(p);
        if !ok(hit) {
            return msg("Seçilen yüz açık değil.");
        }
        // The tangent steps: along the face's plane.
        let mut tangents: Vec<Pos> = Vec::new();
        for a in 0..3 {
            if a != axis {
                for s in [-1, 1] {
                    let mut d = [0; 3];
                    d[a] = s;
                    tangents.push(d);
                }
            }
        }
        let cap = self.limit.min(MAX_WORK);
        let mut seen: HashSet<Pos> = HashSet::new();
        seen.insert(hit);
        let mut queue: VecDeque<Pos> = VecDeque::new();
        queue.push_back(hit);
        let mut surface_cells: Vec<(Pos, String)> = Vec::new();
        while let Some(p) = queue.pop_front() {
            if surface_cells.len() >= cap {
                break;
            }
            if let Some(b) = world.block(p) {
                surface_cells.push((p, block_string(&b)));
            }
            for t in &tangents {
                let q = add(p, *t);
                if seen.insert(q) && ok(q) {
                    queue.push_back(q);
                }
            }
        }
        let passable = |p: Pos| world.block(p).is_some_and(|b| !b.solid);
        let back = mul(f, -1);
        let mut edits: BTreeMap<Pos, String> = BTreeMap::new();
        for (s, src) in &surface_cells {
            let thin = self.displace && passable(add(*s, back));
            if !input.secondary {
                let mut free = 0;
                for k in 1..=self.count {
                    if passable(add(*s, mul(f, k))) {
                        free = k;
                    } else {
                        break;
                    }
                }
                if free == 0 {
                    continue;
                }
                if thin {
                    if free == self.count {
                        edits.insert(*s, AIR.to_string());
                        edits.insert(add(*s, mul(f, self.count)), src.clone());
                    }
                } else {
                    for k in 1..=free {
                        edits.insert(add(*s, mul(f, k)), src.clone());
                    }
                }
            } else if thin {
                if (1..=self.count).all(|k| passable(add(*s, mul(back, k)))) {
                    edits.insert(*s, AIR.to_string());
                    edits.insert(add(*s, mul(back, self.count)), src.clone());
                }
            } else {
                for k in 0..self.count {
                    let q = add(*s, mul(back, k));
                    if world.block(q).is_some_and(|b| b.solid && self.compare.matches(&seed, &b)) {
                        edits.insert(q, AIR.to_string());
                    } else {
                        break;
                    }
                }
            }
        }
        finish(edits)
    }
}

/// Every tool of this module, in the order Axiom's documentation lists them.
pub fn tools() -> Vec<Box<dyn Tool>> {
    let mut out: Vec<Box<dyn Tool>> = Vec::new();
    out.push(Box::new(Floodfill::default()));
    out.push(Box::new(FluidBall::default()));
    out.push(Box::new(Elevation::default()));
    out.push(Box::new(Slope::default()));
    out.push(Box::new(Smooth::default()));
    out.push(Box::new(Distort::default()));
    out.push(Box::new(Roughen::default()));
    out.push(Box::new(Shatter::default()));
    out.push(Box::new(Extrude::default()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    struct World(HashMap<Pos, Block>);

    fn blk(id: &str, solid: bool) -> Block {
        Block { id: format!("minecraft:{id}"), state: String::new(), solid }
    }

    impl World {
        /// Air in a 25 block cube around the origin.
        fn empty() -> Self {
            let mut m = HashMap::new();
            for x in -12..=12 {
                for y in -12..=12 {
                    for z in -12..=12 {
                        m.insert([x, y, z], blk("air", false));
                    }
                }
            }
            World(m)
        }

        /// Stone up to and including `top`, air above.
        fn ground(top: i32) -> Self {
            let mut w = World::empty();
            for x in -12..=12 {
                for z in -12..=12 {
                    for y in -12..=top {
                        w.0.insert([x, y, z], blk("stone", true));
                    }
                }
            }
            w
        }

        fn put(&mut self, p: Pos, b: Block) {
            self.0.insert(p, b);
        }

        fn apply(&mut self, edits: &BTreeMap<Pos, String>) {
            for (p, s) in edits {
                let id = s.split('[').next().unwrap_or(s).to_string();
                let solid = !id.ends_with("air");
                self.0.insert(*p, Block { id, state: String::new(), solid });
            }
        }
    }

    impl Blocks for World {
        fn block(&self, at: Pos) -> Option<Block> {
            self.0.get(&at).cloned()
        }
    }

    fn input(points: &[Pos], face: Pos, secondary: bool, active: &str) -> Input {
        Input { stroke: points.to_vec(), face, active: active.to_string(), secondary, ..Input::default() }
    }

    fn edits(o: Outcome) -> BTreeMap<Pos, String> {
        match o {
            Outcome::Edit(e) => e,
            other => panic!("expected an Edit, got {other:?}"),
        }
    }

    const UP: Pos = [0, 1, 0];

    #[test]
    fn the_list_is_in_docs_order() {
        let ids: Vec<&str> = tools().iter().map(|t| t.id()).collect();
        assert_eq!(
            ids,
            vec!["taskin", "sivitop", "yukselti", "egim", "yumusat", "bozma", "puruz", "parcala", "cikinti"]
        );
    }

    #[test]
    fn floodfill_spreads_across_and_down_to_its_limit() {
        let w = World::ground(-1);
        let mut t = Floodfill::default();
        t.set("limit", Some("10")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, -1, 0]], UP, false, "minecraft:water")));
        assert_eq!(e.len(), 10);
        // Down is on: nothing above the starting layer, and the floor is solid.
        assert!(e.keys().all(|p| p[1] == 0));
        assert!(e.values().all(|v| v == "minecraft:water"));
        // Up instead of down reaches the layers above.
        t.set("yukari", Some("ac")).unwrap();
        t.set("asagi", Some("kapat")).unwrap();
        t.set("limit", Some("30")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, -1, 0]], UP, false, "minecraft:water")));
        assert_eq!(e.len(), 30);
        assert!(e.keys().all(|p| p[1] >= 0));
        assert!(e.keys().any(|p| p[1] > 0));
        // No horizontal spread: just the column.
        t.set("yatay", Some("kapat")).unwrap();
        t.set("limit", Some("1000")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, -1, 0]], UP, false, "minecraft:water")));
        assert!(e.keys().all(|p| p[0] == 0 && p[2] == 0));
    }

    #[test]
    fn floodfill_stays_inside_walls() {
        let mut w = World::ground(-1);
        // A 3x3 room of walls two high around a 1x1 cell.
        for x in -1..=1 {
            for z in -1..=1 {
                if (x, z) != (0, 0) {
                    for y in 0..=1 {
                        w.put([x, y, z], blk("stone", true));
                    }
                }
            }
        }
        w.put([0, 2, 0], blk("stone", true));
        let t = Floodfill { down: false, ..Floodfill::default() };
        let e = edits(t.apply(&w, &input(&[[0, -1, 0]], UP, false, "minecraft:water")));
        assert_eq!(e.keys().copied().collect::<Vec<_>>(), vec![[0, 0, 0], [0, 1, 0]]);
    }

    #[test]
    fn fluid_ball_flows_a_diamond_over_flat_ground() {
        let w = World::ground(-1);
        let mut t = FluidBall::default();
        t.set("yaricap", Some("0")).unwrap();
        t.set("akis_uzunlugu", Some("3")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], UP, false, "")));
        assert_eq!(e.len(), 25);
        assert_eq!(e[&[0, 0, 0]], "minecraft:water");
        assert_eq!(e[&[3, 0, 0]], "minecraft:water[level=3]");
        assert!(!e.contains_key(&[4, 0, 0]));
        t.set("sivi", Some("lav")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], UP, false, "")));
        assert_eq!(e[&[1, 0, 0]], "minecraft:lava[level=1]");
    }

    #[test]
    fn fluid_ball_runs_towards_a_drop_and_falls() {
        let mut w = World::ground(-1);
        w.put([2, -1, 0], blk("air", false));
        let mut t = FluidBall::default();
        t.set("yaricap", Some("0")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], UP, false, "")));
        assert_eq!(e[&[2, -1, 0]], "minecraft:water[level=8]");
        assert!(e.contains_key(&[1, 0, 0]));
        assert!(!e.contains_key(&[-1, 0, 0]), "it ran the other way");
    }

    #[test]
    fn fluid_ball_removes_unsourced_flow() {
        let mut w = World::ground(-1);
        w.put([1, 0, 0], Block { id: "minecraft:water".into(), state: "level=2".into(), solid: false });
        let mut t = FluidBall::default();
        t.set("yaricap", Some("1")).unwrap();
        t.set("kaynaksiz_sil", Some("ac")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], UP, false, "")));
        assert_eq!(e[&[1, 0, 0]], AIR);
    }

    #[test]
    fn elevation_raises_with_the_right_materials() {
        let mut w = World::ground(0);
        w.put([0, 0, 0], blk("grass_block", true));
        w.put([0, -1, 0], blk("dirt", true));
        let mut t = Elevation::default();
        t.set("yaricap", Some("0")).unwrap();
        t.set("y_siniri", Some("4")).unwrap();
        t.set("dusus", Some("yok")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], UP, false, "")));
        assert_eq!(e[&[0, 0, 0]], "minecraft:dirt");
        assert_eq!(e[&[0, 1, 0]], "minecraft:dirt");
        assert_eq!(e[&[0, 2, 0]], "minecraft:grass_block");
        assert!(!e.contains_key(&[0, 3, 0]));
    }

    #[test]
    fn elevation_lowers_flattens_and_covers_the_brush() {
        let w = World::ground(0);
        let mut t = Elevation::default();
        t.set("yaricap", Some("1")).unwrap();
        t.set("y_siniri", Some("4")).unwrap();
        t.set("dusus", Some("yok")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], UP, false, "")));
        assert_eq!(e.keys().filter(|p| p[1] == 2).count(), 9);
        // The other button lowers.
        t.set("yaricap", Some("0")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], UP, true, "")));
        assert_eq!(e[&[0, 0, 0]], AIR);
        assert_eq!(e[&[0, -1, 0]], AIR);
        assert_eq!(e[&[0, -2, 0]], "minecraft:stone");
        // Flatten to the height looked at.
        t.set("mod", Some("duzle")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 2, 0]], UP, false, "")));
        assert_eq!(e[&[0, 2, 0]], "minecraft:stone");
        assert!(!e.contains_key(&[0, 3, 0]));
    }

    #[test]
    fn slope_builds_a_ramp_between_two_points() {
        let w = World::ground(0);
        let mut t = Slope::default();
        t.set("yaricap", Some("0")).unwrap();
        t.set("y_siniri", Some("6")).unwrap();
        t.set("hedef", Some("0 0 0")).unwrap();
        // One block up for each block along: at x = 4 the plane is 4 high.
        let e = edits(t.apply(&w, &input(&[[4, 4, 0]], UP, false, "")));
        assert_eq!(e[&[4, 4, 0]], "minecraft:stone");
        assert!(!e.contains_key(&[4, 5, 0]));
        // No two points, no slope.
        t.set("hedef", None).unwrap();
        assert!(matches!(t.apply(&w, &input(&[[4, 4, 0]], UP, false, "")), Outcome::Message(_)));
        assert!(t.set("hedef", Some("1 2")).is_err());
    }

    #[test]
    fn smooth_keeps_the_mass_and_shaves_a_spike() {
        let mut w = World::ground(0);
        w.put([0, 1, 0], blk("stone", true));
        w.put([0, 2, 0], blk("stone", true));
        let t = Smooth::default();
        let inp = input(&[[0, 1, 0]], UP, false, "");
        let region = brush([0, 1, 0], 5, Shape::Sphere);
        let count = |w: &World| region.iter().filter(|p| w.block(**p).is_some_and(|b| b.solid)).count();
        let before = count(&w);
        let e = edits(t.apply(&w, &inp));
        assert_eq!(e[&[0, 2, 0]], AIR);
        let mut after = World(w.0.clone());
        after.apply(&e);
        assert_eq!(count(&after), before);
    }

    #[test]
    fn smooth_leaves_flat_ground_alone_and_ratio_removes_mass() {
        let w = World::ground(0);
        let mut t = Smooth::default();
        let inp = input(&[[0, 0, 0]], UP, false, "");
        assert!(matches!(t.apply(&w, &inp), Outcome::Message(_)));
        t.set("oran", Some("90")).unwrap();
        let e = edits(t.apply(&w, &inp));
        assert!(e.values().all(|v| v == AIR));
        assert!(!e.is_empty());
    }

    #[test]
    fn distort_moves_blocks_deterministically() {
        let w = World::ground(0);
        let mut t = Distort::default();
        t.set("yaricap", Some("3")).unwrap();
        t.set("olcek", Some("3")).unwrap();
        let inp = input(&[[0, 0, 0]], UP, false, "");
        let a = edits(t.apply(&w, &inp));
        assert!(!a.is_empty());
        assert!(a.values().all(|v| v == AIR || v == "minecraft:stone"));
        assert_eq!(a, edits(t.apply(&w, &inp)));
        // A tiny distance moves nothing.
        t.set("mesafe", Some("0.1")).unwrap();
        assert!(matches!(t.apply(&w, &inp), Outcome::Message(_)));
    }

    #[test]
    fn roughen_digs_and_builds_at_the_surface() {
        let w = World::ground(0);
        let mut t = Roughen::default();
        t.set("yaricap", Some("3")).unwrap();
        t.set("yuz", Some("4")).unwrap();
        let inp = input(&[[0, 0, 0]], UP, false, "");
        t.set("oran", Some("1")).unwrap();
        let e = edits(t.apply(&w, &inp));
        assert_eq!(e[&[0, 0, 0]], AIR);
        assert_eq!(e[&[0, 1, 0]], "minecraft:stone");
        // Nothing buried is touched.
        assert!(!e.contains_key(&[0, -2, 0]));
        t.set("oran", Some("0")).unwrap();
        assert!(matches!(t.apply(&w, &inp), Outcome::Message(_)));
        t.set("oran", Some("0.5")).unwrap();
        let half = edits(t.apply(&w, &inp));
        assert!(!half.is_empty() && half.len() < 100);
    }

    #[test]
    fn shatter_cracks_solid_blocks_only_and_along_its_axes() {
        let w = World::ground(12);
        let mut t = Shatter::default();
        t.set("yaricap", Some("4")).unwrap();
        t.set("olcek", Some("4")).unwrap();
        t.set("genislik", Some("3")).unwrap();
        let inp = input(&[[0, 0, 0]], UP, false, "minecraft:glass");
        let e = edits(t.apply(&w, &inp));
        assert!(e.values().all(|v| v == AIR));
        // With the active block the cracks are made of it.
        t.set("etkin_blok", Some("ac")).unwrap();
        let g = edits(t.apply(&w, &inp));
        assert_eq!(g.keys().collect::<Vec<_>>(), e.keys().collect::<Vec<_>>());
        assert!(g.values().all(|v| v == "minecraft:glass"));
        // On the x axis alone a crack is a whole plane across x.
        t.set("eksen", Some("x")).unwrap();
        let x = edits(t.apply(&w, &inp));
        let xs: BTreeSet<i32> = x.keys().map(|p| p[0]).collect();
        for p in brush([0, 0, 0], 4, Shape::Sphere) {
            assert_eq!(x.contains_key(&p), xs.contains(&p[0]));
        }
        assert!(t.set("eksen", Some("q")).is_err());
        // Air is never cracked.
        let air = World::empty();
        assert!(matches!(t.apply(&air, &inp), Outcome::Message(_)));
    }

    #[test]
    fn extrude_pushes_the_whole_face_out_and_pulls_it_in() {
        let w = World::ground(0);
        let mut t = Extrude::default();
        t.set("adet", Some("2")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], UP, false, "")));
        assert_eq!(e.len(), 25 * 25 * 2);
        assert_eq!(e[&[0, 1, 0]], "minecraft:stone");
        assert_eq!(e[&[0, 2, 0]], "minecraft:stone");
        assert!(!e.contains_key(&[0, 3, 0]));
        // The limit cuts the surface.
        t.set("limit", Some("10")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], UP, false, "")));
        assert_eq!(e.len(), 20);
        // The other button pulls in.
        t.set("limit", Some("100000")).unwrap();
        t.set("adet", Some("1")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], UP, true, "")));
        assert_eq!(e.len(), 625);
        assert!(e.values().all(|v| v == AIR));
        assert_eq!(e[&[0, 0, 0]], AIR);
    }

    #[test]
    fn extrude_displaces_a_thin_wall_and_respects_compare() {
        let mut w = World::empty();
        for x in -3..=3 {
            for z in -3..=3 {
                w.put([x, 0, z], blk("stone", true));
            }
        }
        w.put([4, 0, 0], blk("dirt", true));
        let mut t = Extrude::default();
        let inp = input(&[[0, 0, 0]], UP, false, "");
        // Same block only: the dirt is left out; a copy adds a layer.
        let copy = edits(t.apply(&w, &inp));
        assert_eq!(copy.len(), 49);
        assert!(!copy.contains_key(&[4, 1, 0]));
        // Displaced: the wall moves, so it also clears the old one.
        t.set("yer_degistir", Some("ac")).unwrap();
        let moved = edits(t.apply(&w, &inp));
        assert_eq!(moved.len(), 98);
        assert_eq!(moved[&[0, 0, 0]], AIR);
        assert_eq!(moved[&[0, 1, 0]], "minecraft:stone");
        // Any solid block takes the dirt too.
        t.set("karsilastir", Some("kati")).unwrap();
        let all = edits(t.apply(&w, &inp));
        assert_eq!(all[&[4, 1, 0]], "minecraft:dirt");
        // A block with no open face gives a message.
        let buried = World::ground(5);
        assert!(matches!(t.apply(&buried, &input(&[[0, 0, 0]], UP, false, "")), Outcome::Message(_)));
    }

    #[test]
    fn unloaded_blocks_are_left_alone() {
        // Only the one block is loaded: nothing can be changed around it.
        let mut m = HashMap::new();
        m.insert([0, 0, 0], blk("stone", true));
        let w = World(m);
        let inp = input(&[[0, 0, 0]], UP, false, "minecraft:water");
        for mut t in tools() {
            if let Outcome::Edit(e) = t.apply(&w, &inp) {
                assert!(e.keys().all(|p| *p == [0, 0, 0]), "{} touched an unloaded block", t.id());
            }
            // Options must reject unknown names.
            assert!(t.set("yok_boyle_ayar", Some("1")).is_err());
        }
    }
}
