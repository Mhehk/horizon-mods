// Eksen — 16.10.1 (mihehk): the painting tools. Written from scratch after the
// behaviour Axiom's documentation describes (.build-tools/axiom-docs/axiom-docs.md:
// "Painting Tools", "Painter", "Noise Painter", "Biome Painter", "Script Brush",
// "Gradient Painter", "Palette", "Active Block"); no code, names or images of
// Axiom are used.
//
// A painting tool replaces blocks that are already there. By the docs it only
// touches solid blocks (never air) unless the player widens it with masks;
// here `kati` (solid only, on by default) stands in for that mask.
//
// Tools (docs order): boyaci (Painter), gurultu (Noise Painter), biyom (Biome
// Painter), betik (Script Brush: a small Lua-like subset, see `script`), gecis
// (Gradient Painter).
//
// Palettes are written `stone:3,andesite:1` (block, optional weight; commas in
// `[...]` block states are kept together). A block without a namespace is a
// `minecraft:` block.
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Mutex;

use super::eksen_core::*;

/// The most blocks one stroke may touch.
pub const MAX_BLOCKS: usize = 1_000_000;
/// The largest brush radius an option accepts.
const MAX_RADIUS: i32 = 50;
/// The world's build height (biome boxes and the script's height lookup).
const WORLD_MIN_Y: i32 = -64;
const WORLD_MAX_Y: i32 = 319;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn msg(text: &str) -> Outcome {
    Outcome::Message(vec![text.to_string()])
}

fn too_big() -> String {
    format!("Fırça darbesi çok büyük (en fazla {MAX_BLOCKS} blok).")
}

/// A required, non-empty text value.
fn text(value: Option<&str>) -> Result<&str, String> {
    value.map(str::trim).filter(|v| !v.is_empty()).ok_or_else(|| "Bir değer gerekli.".to_string())
}

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A repeatable pseudo-random number in `0.0..1.0` for a position.
fn hash_pos(seed: u64, p: Pos) -> f64 {
    let mut h = mix(seed ^ 0xA076_1D64_78BD_642F);
    for v in p {
        h = mix(h ^ (v as i64 as u64));
    }
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// `dirt` becomes `minecraft:dirt`; a state (`[...]`) is kept.
fn norm_block(name: &str) -> String {
    let name = name.trim();
    let head = name.split('[').next().unwrap_or(name);
    if head.contains(':') {
        name.to_string()
    } else {
        format!("minecraft:{name}")
    }
}

/// `minecraft:oak_stairs[facing=east]` into its id and state text.
fn split_block(s: &str) -> (&str, &str) {
    match s.find('[') {
        Some(i) => (&s[..i], s[i + 1..].trim_end_matches(']')),
        None => (s, ""),
    }
}

fn with_state(id: &str, state: &str) -> String {
    if state.is_empty() {
        id.to_string()
    } else {
        format!("{id}[{state}]")
    }
}

fn local_name(id: &str) -> &str {
    id.rsplit(':').next().unwrap_or(id)
}

/// The last word of a block's name: `stairs` for `minecraft:oak_stairs`.
fn kind_of(id: &str) -> &str {
    let n = local_name(id);
    n.rsplit('_').next().unwrap_or(n)
}

/// The kinds that come in variants (Type Replace applies to them).
const VARIANT_KINDS: &[&str] = &["stairs", "slab", "fence", "wall", "gate", "trapdoor", "door", "button", "plate", "sign"];

fn is_air_id(id: &str) -> bool {
    matches!(id, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
}

/// What to place over `old` when painting `new`: Type Replace swaps the type
/// of a variant block and keeps its properties (blocks of another kind are
/// left alone); Copy Properties carries the old block's properties over when
/// `new` names none and both are of one kind.
// Inferred: "same kind" is the last word of the id (stairs, slab, log ...),
// since the tool cannot see the real property lists.
fn resolve(old: &Block, new: &str, copy: bool, replace: bool) -> Option<String> {
    let (nid, nstate) = split_block(new);
    let same = kind_of(&old.id) == kind_of(nid);
    if replace {
        if !(same && VARIANT_KINDS.iter().any(|k| *k == kind_of(nid))) {
            return None;
        }
        return Some(with_state(nid, &old.state));
    }
    if copy && nstate.is_empty() && same && !old.state.is_empty() {
        return Some(with_state(nid, &old.state));
    }
    Some(new.to_string())
}

const FACES: [Pos; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

/// Mask Surface: the block touches a non-solid block.
fn exposed(world: &dyn Blocks, p: Pos) -> bool {
    FACES.iter().any(|d| world.block([p[0] + d[0], p[1] + d[1], p[2] + d[2]]).is_some_and(|b| !b.solid))
}

/// Every block a stroke's brushes cover, refusing past the work cap.
fn capped_stroke(points: &[Pos], radius: i32, shape: Shape) -> Result<BTreeSet<Pos>, String> {
    let mut out = BTreeSet::new();
    for p in points {
        for b in brush(*p, radius, shape) {
            out.insert(b);
            if out.len() > MAX_BLOCKS {
                return Err(too_big());
            }
        }
    }
    Ok(out)
}

/// Like `capped_stroke`, with each block's squared distance to the nearest
/// brush centre.
fn nearest_map(points: &[Pos], radius: i32, shape: Shape) -> Result<BTreeMap<Pos, i64>, String> {
    let mut out: BTreeMap<Pos, i64> = BTreeMap::new();
    for c in points {
        for b in brush(*c, radius, shape) {
            let d = [0, 1, 2].map(|k| i64::from(b[k] - c[k]));
            let d2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
            let e = out.entry(b).or_insert(d2);
            if d2 < *e {
                *e = d2;
            }
            if out.len() > MAX_BLOCKS {
                return Err(too_big());
            }
        }
    }
    Ok(out)
}

/// The options every painter shares.
#[derive(Clone, Debug)]
struct Common {
    radius: i32,
    shape: Shape,
    /// Only solid blocks are painted (the documented default).
    solid: bool,
    /// Mask Surface.
    surface: bool,
    /// Copy Properties.
    copy: bool,
    /// Type Replace.
    replace: bool,
}

impl Common {
    fn new() -> Self {
        // Inferred: the docs give no default radius.
        Common { radius: 4, shape: Shape::Sphere, solid: true, surface: false, copy: false, replace: false }
    }

    /// Handles a shared option; `None` if `option` is not one of them.
    fn set(&mut self, option: &str, value: Option<&str>) -> Option<Result<(), String>> {
        Some(match option {
            "yaricap" | "yarıçap" => number::<i32>(value, 0, MAX_RADIUS).map(|v| self.radius = v),
            "sekil" | "şekil" => text(value)
                .and_then(|w| Shape::parse(w).ok_or_else(|| format!("Bilinmeyen şekil: {w}")))
                .map(|s| self.shape = s),
            "kati" | "katı" => toggle(&mut self.solid, value),
            "yuzey" | "yüzey" => toggle(&mut self.surface, value),
            "ozellik" | "özellik" => toggle(&mut self.copy, value),
            "tur_degistir" => toggle(&mut self.replace, value),
            _ => return None,
        })
    }

    fn describe(&self) -> Vec<String> {
        vec![
            format!("yaricap: {}", self.radius),
            format!("sekil: {}", self.shape.name()),
            format!("kati: {}", on_off(self.solid)),
            format!("yuzey: {}", on_off(self.surface)),
            format!("ozellik: {}", on_off(self.copy)),
            format!("tur_degistir: {}", on_off(self.replace)),
        ]
    }
}

/// The painting loop all painters share: for each position, skip air, apply
/// the solid and surface masks, ask `choose` for the new block and resolve
/// Copy Properties / Type Replace. Never writes a block over itself.
fn paint(
    world: &dyn Blocks,
    positions: impl Iterator<Item = Pos>,
    c: &Common,
    mut choose: impl FnMut(Pos, &Block) -> Option<String>,
) -> BTreeMap<Pos, String> {
    let mut out = BTreeMap::new();
    for p in positions {
        let Some(old) = world.block(p) else { continue };
        if old.air() || (c.solid && !old.solid) {
            continue;
        }
        if c.surface && !exposed(world, p) {
            continue;
        }
        let Some(new) = choose(p, &old) else { continue };
        let Some(new) = resolve(&old, &new, c.copy, c.replace) else { continue };
        if new == block_string(&old) {
            continue;
        }
        out.insert(p, new);
    }
    out
}

fn finish(edits: BTreeMap<Pos, String>) -> Outcome {
    if edits.is_empty() {
        msg("Boyanacak blok bulunamadı.")
    } else {
        Outcome::Edit(edits)
    }
}

// ---------------------------------------------------------------------------
// Palette and gradients
// ---------------------------------------------------------------------------

/// Splits at commas that are not inside `[...]`.
fn split_top(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in text.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            ',' if depth <= 0 => {
                out.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&text[start..]);
    out
}

/// Blocks with weights: `stone:3,andesite:1`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Palette {
    text: String,
    entries: Vec<(String, f64)>,
}

impl Palette {
    pub fn parse(src: &str) -> Result<Palette, String> {
        let mut entries = Vec::new();
        for part in split_top(src) {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let weighted = part.rfind(':').and_then(|i| part[i + 1..].trim().parse::<f64>().ok().map(|w| (i, w)));
            let (name, w) = match weighted {
                Some((i, w)) if !part[i + 1..].contains(']') => (&part[..i], w),
                _ => (part, 1.0),
            };
            let name = name.trim();
            if name.is_empty() || name.chars().any(char::is_whitespace) {
                return Err(format!("Geçersiz blok: {part}"));
            }
            if !w.is_finite() || w < 0.0 {
                return Err(format!("Geçersiz ağırlık: {part}"));
            }
            entries.push((norm_block(name), w));
        }
        Ok(Palette { text: src.trim().to_string(), entries })
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The palette itself, or just the active block if it is empty.
    fn with_active(&self, active: &str) -> Palette {
        if self.entries.is_empty() && !active.trim().is_empty() {
            Palette { text: String::new(), entries: vec![(norm_block(active), 1.0)] }
        } else {
            self.clone()
        }
    }

    fn blocks(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.0.clone()).collect()
    }

    /// The block at `u` (in `0.0..1.0`) of the weights laid end to end.
    fn pick(&self, u: f64) -> &str {
        let total: f64 = self.entries.iter().map(|e| e.1).sum();
        let mut left = u * total;
        for (b, w) in &self.entries {
            if left < *w {
                return b;
            }
            left -= *w;
        }
        self.entries.last().map(|e| e.0.as_str()).unwrap_or(AIR)
    }

    fn shown(&self) -> String {
        if self.text.is_empty() {
            "(etkin blok)".to_string()
        } else {
            self.text.clone()
        }
    }
}

/// How a gradient moves from one block to the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interp {
    /// Hard bands.
    Nearest,
    /// A smooth mix (the blocks of two neighbours dither).
    Linear,
    /// Eased: quick in the middle, slow at both ends.
    Bezier,
}

impl Interp {
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "yakin" | "yakın" | "nearest" => Interp::Nearest,
            "dogrusal" | "doğrusal" | "linear" => Interp::Linear,
            "bezier" => Interp::Bezier,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Interp::Nearest => "yakın",
            Interp::Linear => "doğrusal",
            Interp::Bezier => "bezier",
        }
    }
}

/// The block at `t` (0 to 1) along a gradient of `blocks`; `u` is a random
/// number in `0..1` that dithers the soft interpolations.
fn gradient_pick(blocks: &[String], t: f64, interp: Interp, u: f64) -> &str {
    let n = blocks.len();
    if n == 0 {
        return AIR;
    }
    if n == 1 {
        return &blocks[0];
    }
    let t = t.clamp(0.0, 1.0);
    // Inferred: the Bezier ease is the smoothstep curve.
    let t = if interp == Interp::Bezier { t * t * (3.0 - 2.0 * t) } else { t };
    let i = if interp == Interp::Nearest {
        ((t * n as f64) as usize).min(n - 1)
    } else {
        let f = t * (n - 1) as f64;
        let lo = (f.floor() as usize).min(n - 1);
        let frac = f - lo as f64;
        if lo + 1 < n && u < frac {
            lo + 1
        } else {
            lo
        }
    };
    &blocks[i]
}

// ---------------------------------------------------------------------------
// Painter
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PaintMode {
    Active,
    Gradient,
}

/// The Painter: paints solid blocks under the brush with the active block or
/// a weighted palette, or (gradient mode) a gradient around each brush centre.
pub struct Painter {
    c: Common,
    mode: PaintMode,
    palette: Palette,
    seed: u64,
    /// Gradient mode: leave blocks that are already one of the gradient's.
    merge: bool,
    interp: Interp,
}

impl Painter {
    pub fn new() -> Self {
        Painter {
            c: Common::new(),
            mode: PaintMode::Active,
            palette: Palette::default(),
            seed: 0,
            merge: false,
            // Inferred: the docs name no default interpolation.
            interp: Interp::Linear,
        }
    }
}

impl Tool for Painter {
    fn id(&self) -> &'static str {
        "boyaci"
    }

    fn name(&self) -> &'static str {
        "Boyacı"
    }

    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        if let Some(r) = self.c.set(option, value) {
            return r;
        }
        match option {
            "mod" => {
                self.mode = match text(value)? {
                    "etkin" | "aktif" | "active" => PaintMode::Active,
                    "gecis" | "geçiş" | "gradient" => PaintMode::Gradient,
                    "doseme" | "damga" | "tiled" | "stamped" => {
                        return Err("Pano modları (döşeme, damga) desteklenmiyor.".to_string())
                    }
                    w => return Err(format!("Bilinmeyen mod: {w} (etkin, gecis)")),
                };
                Ok(())
            }
            "palet" => {
                self.palette = match value.map(str::trim).filter(|v| !v.is_empty()) {
                    Some(v) => Palette::parse(v)?,
                    None => Palette::default(),
                };
                Ok(())
            }
            "tohum" => {
                self.seed = number::<u64>(value, 0, u64::MAX)?;
                Ok(())
            }
            "birlestir" | "birleştir" => toggle(&mut self.merge, value),
            "interpolasyon" => {
                let w = text(value)?;
                self.interp = Interp::parse(w).ok_or_else(|| format!("Bilinmeyen geçiş türü: {w}"))?;
                Ok(())
            }
            _ => Err(format!("Bilinmeyen seçenek: {option}")),
        }
    }

    fn describe(&self) -> Vec<String> {
        let mut v = self.c.describe();
        v.push(format!("mod: {}", if self.mode == PaintMode::Active { "etkin" } else { "gecis" }));
        v.push(format!("palet: {}", self.palette.shown()));
        v.push(format!("tohum: {}", self.seed));
        v.push(format!("birlestir: {}", on_off(self.merge)));
        v.push(format!("interpolasyon: {}", self.interp.name()));
        v
    }

    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        if input.stroke.is_empty() {
            return msg("Fırça darbesi yok.");
        }
        let pal = self.palette.with_active(&input.active);
        if pal.is_empty() {
            return msg("Etkin blok yok.");
        }
        let edits = match self.mode {
            PaintMode::Active => {
                let targets = match capped_stroke(&input.stroke, self.c.radius, self.c.shape) {
                    Ok(t) => t,
                    Err(m) => return msg(&m),
                };
                paint(world, targets.iter().copied(), &self.c, |p, _| Some(pal.pick(hash_pos(self.seed, p)).to_string()))
            }
            PaintMode::Gradient => {
                let near = match nearest_map(&input.stroke, self.c.radius, self.c.shape) {
                    Ok(t) => t,
                    Err(m) => return msg(&m),
                };
                let blocks = pal.blocks();
                let reach = f64::from(self.c.radius) + 0.5;
                paint(world, near.keys().copied(), &self.c, |p, old| {
                    if self.merge && blocks.iter().any(|b| *b == block_string(old)) {
                        return None;
                    }
                    let t = (near[&p] as f64).sqrt() / reach;
                    Some(gradient_pick(&blocks, t, self.interp, hash_pos(self.seed, p)).to_string())
                })
            }
        };
        finish(edits)
    }
}

// ---------------------------------------------------------------------------
// Noise Painter
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoiseKind {
    Simplex,
    Worley,
    VoronoiEdges,
    Metaball,
    White,
    Splatter,
}

impl NoiseKind {
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "simpleks" | "simplex" => NoiseKind::Simplex,
            "worley" => NoiseKind::Worley,
            "voronoi" | "voronoi_kenar" => NoiseKind::VoronoiEdges,
            "metaball" | "metatop" => NoiseKind::Metaball,
            "beyaz" | "white" => NoiseKind::White,
            "sicrama" | "sıçrama" | "splatter" => NoiseKind::Splatter,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            NoiseKind::Simplex => "simpleks",
            NoiseKind::Worley => "worley",
            NoiseKind::VoronoiEdges => "voronoi",
            NoiseKind::Metaball => "metaball",
            NoiseKind::White => "beyaz",
            NoiseKind::Splatter => "sicrama",
        }
    }
}

/// The noise settings of the Noise Painter.
#[derive(Clone, Debug, PartialEq)]
pub struct NoiseOpts {
    pub kind: NoiseKind,
    /// Computed in 3D (the sides of surfaces too); off: one value per column.
    pub three_d: bool,
    pub scale: f64,
    pub anisotropic: bool,
    /// Per-axis scales, used with `anisotropic`.
    pub axis_scale: [f64; 3],
    pub octaves: u32,
    pub lacunarity: f64,
    pub gain: f64,
    pub seed: u64,
    /// Cellular noises: how far seed points leave their grid (0 to 1).
    pub jitter: f64,
    /// Worley: the weights of the 1st, 2nd and 3rd closest points.
    pub weights: [f64; 3],
    /// Metaball: the radius around each point, in cells.
    pub range: f64,
}

impl Default for NoiseOpts {
    // Inferred: the docs give no defaults for scale, octaves, lacunarity,
    // gain or the Worley weights; these are the usual ones.
    fn default() -> Self {
        NoiseOpts {
            kind: NoiseKind::Simplex,
            three_d: true,
            scale: 8.0,
            anisotropic: false,
            axis_scale: [8.0; 3],
            octaves: 1,
            lacunarity: 2.0,
            gain: 0.5,
            seed: 0,
            jitter: 1.0,
            weights: [1.0, 0.0, 0.0],
            range: 1.0,
        }
    }
}

impl NoiseOpts {
    fn scales(&self) -> [f64; 3] {
        if self.anisotropic {
            self.axis_scale
        } else {
            [self.scale; 3]
        }
    }
}

/// The seed point of a cell.
fn feature(cell: Pos, seed: u64, jitter: f64) -> [f64; 3] {
    [0u64, 1, 2].map(|k| {
        let h = hash_pos(seed.wrapping_add(k + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15), cell);
        cell[k as usize] as f64 + 0.5 + (h - 0.5) * jitter
    })
}

/// Distances from `c` to the seed points of the 27 cells around it.
fn feature_dists(c: [f64; 3], seed: u64, jitter: f64) -> [f64; 27] {
    let base = c.map(|v| v.floor() as i32);
    let mut out = [0.0; 27];
    let mut i = 0;
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                let f = feature([base[0] + dx, base[1] + dy, base[2] + dz], seed, jitter);
                let d2 = (f[0] - c[0]).powi(2) + (f[1] - c[1]).powi(2) + (f[2] - c[2]).powi(2);
                out[i] = d2.sqrt();
                i += 1;
            }
        }
    }
    out
}

/// One octave of noise at `c` (already scaled), in `0.0..=1.0`.
fn base_noise(o: &NoiseOpts, c: [f64; 3], seed: u64) -> f64 {
    match o.kind {
        NoiseKind::Simplex => (noise3(c, 1.0, seed) + 1.0) / 2.0,
        NoiseKind::Splatter => {
            let cell = c.map(|v| v.floor() as i32);
            ((noise3(c, 1.0, seed) + 1.0) / 2.0 + (hash_pos(seed ^ 0x5151, cell) - 0.5) * 0.5).clamp(0.0, 1.0)
        }
        NoiseKind::Worley => {
            let mut d = feature_dists(c, seed, o.jitter);
            d.sort_by(|a, b| a.total_cmp(b));
            (1.0 - (o.weights[0] * d[0] + o.weights[1] * d[1] + o.weights[2] * d[2])).clamp(0.0, 1.0)
        }
        NoiseKind::VoronoiEdges => {
            let mut d = feature_dists(c, seed, o.jitter);
            d.sort_by(|a, b| a.total_cmp(b));
            // High where the closest and second closest points are almost
            // equally far: the cell borders.
            1.0 - ((d[1] - d[0]) * 2.0).clamp(0.0, 1.0)
        }
        NoiseKind::Metaball => {
            let d = feature_dists(c, seed, o.jitter);
            let s: f64 = d
                .iter()
                .map(|v| {
                    let k = (1.0 - v / o.range).max(0.0);
                    k * k
                })
                .sum();
            s.min(1.0)
        }
        NoiseKind::White => 0.0,
    }
}

/// The noise at block `p`, in `0.0..=1.0`.
pub fn noise_value(o: &NoiseOpts, p: Pos) -> f64 {
    if o.kind == NoiseKind::White {
        return hash_pos(o.seed, [p[0], if o.three_d { p[1] } else { 0 }, p[2]]);
    }
    let sc = o.scales();
    let q = [p[0] as f64 + 0.5, if o.three_d { p[1] as f64 + 0.5 } else { 0.0 }, p[2] as f64 + 0.5];
    let (mut sum, mut amp, mut norm, mut freq) = (0.0, 1.0, 0.0, 1.0);
    for oct in 0..o.octaves.max(1) {
        let c = [0, 1, 2].map(|k| q[k] / sc[k] * freq);
        sum += amp * base_noise(o, c, o.seed.wrapping_add(u64::from(oct) * 0x9E37));
        norm += amp;
        amp *= o.gain;
        freq *= o.lacunarity;
    }
    if norm > 0.0 {
        (sum / norm).clamp(0.0, 1.0)
    } else {
        0.5
    }
}

/// The sorted noise values of a fixed lattice, to turn a value into its rank
/// (so percentages hold whatever the noise's own distribution is).
fn build_cdf(o: &NoiseOpts) -> Vec<f64> {
    let sc = o.scales();
    let step = (((sc[0] + sc[1] + sc[2]) / 3.0) * 0.75).round().max(1.0) as i32;
    let mut v = Vec::with_capacity(1728);
    for i in -6..6 {
        for j in -6..6 {
            for k in -6..6 {
                v.push(noise_value(o, [i * step, j * step, k * step]));
            }
        }
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v
}

fn rank(cdf: &[f64], v: f64) -> f64 {
    if cdf.is_empty() {
        return v;
    }
    cdf.partition_point(|s| *s < v) as f64 / cdf.len() as f64
}

/// How the palette is spread over the noise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spread {
    /// The weights are percentages of the painted blocks.
    Percent,
    /// The numbers are noise thresholds (0 to 1): a block is used where the
    /// noise is at or below its threshold (lowest first); above them all the
    /// block is left alone.
    Threshold,
}

/// The Noise Painter: picks palette blocks by procedural noise.
pub struct NoisePainter {
    c: Common,
    o: NoiseOpts,
    spread: Spread,
    palette: Palette,
}

impl NoisePainter {
    pub fn new() -> Self {
        NoisePainter { c: Common::new(), o: NoiseOpts::default(), spread: Spread::Percent, palette: Palette::default() }
    }
}

impl Tool for NoisePainter {
    fn id(&self) -> &'static str {
        "gurultu"
    }

    fn name(&self) -> &'static str {
        "Gürültü Boyacısı"
    }

    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        if let Some(r) = self.c.set(option, value) {
            return r;
        }
        let o = &mut self.o;
        match option {
            "gurultu" | "gürültü" => {
                let w = text(value)?;
                o.kind = NoiseKind::parse(w).ok_or_else(|| format!("Bilinmeyen gürültü: {w}"))?;
                Ok(())
            }
            "uc_boyut" => toggle(&mut o.three_d, value),
            "olcek" | "ölçek" => {
                o.scale = number::<f64>(value, 0.1, 1024.0)?;
                Ok(())
            }
            "anizotropik" => toggle(&mut o.anisotropic, value),
            "olcek_x" => {
                o.axis_scale[0] = number::<f64>(value, 0.1, 1024.0)?;
                Ok(())
            }
            "olcek_y" => {
                o.axis_scale[1] = number::<f64>(value, 0.1, 1024.0)?;
                Ok(())
            }
            "olcek_z" => {
                o.axis_scale[2] = number::<f64>(value, 0.1, 1024.0)?;
                Ok(())
            }
            "oktav" => {
                o.octaves = number::<u32>(value, 1, 8)?;
                Ok(())
            }
            "bosluk" | "boşluk" => {
                o.lacunarity = number::<f64>(value, 1.0, 8.0)?;
                Ok(())
            }
            "kazanc" | "kazanç" => {
                o.gain = number::<f64>(value, 0.0, 2.0)?;
                Ok(())
            }
            "tohum" => {
                o.seed = number::<u64>(value, 0, u64::MAX)?;
                Ok(())
            }
            "titresim" | "titreşim" => {
                o.jitter = number::<f64>(value, 0.0, 1.0)?;
                Ok(())
            }
            "w1" => {
                o.weights[0] = number::<f64>(value, 0.0, 4.0)?;
                Ok(())
            }
            "w2" => {
                o.weights[1] = number::<f64>(value, 0.0, 4.0)?;
                Ok(())
            }
            "w3" => {
                o.weights[2] = number::<f64>(value, 0.0, 4.0)?;
                Ok(())
            }
            "menzil" => {
                o.range = number::<f64>(value, 0.1, 1.0)?;
                Ok(())
            }
            "dagitim" | "dağıtım" => {
                self.spread = match text(value)? {
                    "yuzde" | "yüzde" | "percent" => Spread::Percent,
                    "esik" | "eşik" | "threshold" => Spread::Threshold,
                    w => return Err(format!("Bilinmeyen dağıtım: {w} (yuzde, esik)")),
                };
                Ok(())
            }
            "palet" => {
                self.palette = match value.map(str::trim).filter(|v| !v.is_empty()) {
                    Some(v) => Palette::parse(v)?,
                    None => Palette::default(),
                };
                Ok(())
            }
            _ => Err(format!("Bilinmeyen seçenek: {option}")),
        }
    }

    fn describe(&self) -> Vec<String> {
        let o = &self.o;
        let mut v = self.c.describe();
        v.push(format!("gurultu: {}", o.kind.name()));
        v.push(format!("uc_boyut: {}", on_off(o.three_d)));
        v.push(format!("olcek: {}", o.scale));
        v.push(format!("anizotropik: {}", on_off(o.anisotropic)));
        v.push(format!("olcek_x/y/z: {}/{}/{}", o.axis_scale[0], o.axis_scale[1], o.axis_scale[2]));
        v.push(format!("oktav: {}", o.octaves));
        v.push(format!("bosluk: {}", o.lacunarity));
        v.push(format!("kazanc: {}", o.gain));
        v.push(format!("tohum: {}", o.seed));
        v.push(format!("titresim: {}", o.jitter));
        v.push(format!("w1/w2/w3: {}/{}/{}", o.weights[0], o.weights[1], o.weights[2]));
        v.push(format!("menzil: {}", o.range));
        v.push(format!("dagitim: {}", if self.spread == Spread::Percent { "yuzde" } else { "esik" }));
        v.push(format!("palet: {}", self.palette.shown()));
        v
    }

    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        if input.stroke.is_empty() {
            return msg("Fırça darbesi yok.");
        }
        let pal = self.palette.with_active(&input.active);
        if pal.is_empty() {
            return msg("Etkin blok yok.");
        }
        let targets = match capped_stroke(&input.stroke, self.c.radius, self.c.shape) {
            Ok(t) => t,
            Err(m) => return msg(&m),
        };
        let edits = match self.spread {
            Spread::Percent => {
                let cdf = if pal.entries.len() > 1 { build_cdf(&self.o) } else { Vec::new() };
                paint(world, targets.iter().copied(), &self.c, |p, _| {
                    let u = if cdf.is_empty() { 0.0 } else { rank(&cdf, noise_value(&self.o, p)) };
                    Some(pal.pick(u).to_string())
                })
            }
            Spread::Threshold => {
                let mut th = pal.entries.clone();
                th.sort_by(|a, b| a.1.total_cmp(&b.1));
                paint(world, targets.iter().copied(), &self.c, |p, _| {
                    let v = noise_value(&self.o, p);
                    th.iter().find(|e| v <= e.1).map(|e| e.0.clone())
                })
            }
        };
        finish(edits)
    }
}

// ---------------------------------------------------------------------------
// Biome Painter
// ---------------------------------------------------------------------------

/// Joins boxes that touch end to end along `axis` and are the same across
/// the other two.
fn merge_axis(mut boxes: Vec<(Pos, Pos)>, axis: usize) -> Vec<(Pos, Pos)> {
    boxes.sort_by_key(|(lo, hi)| {
        let mut k = [lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]];
        k[axis] = 0;
        k[axis + 3] = 0;
        (k, lo[axis])
    });
    let mut out: Vec<(Pos, Pos)> = Vec::new();
    for (lo, hi) in boxes {
        if let Some((plo, phi)) = out.last_mut() {
            let same = (0..3).all(|k| k == axis || (plo[k] == lo[k] && phi[k] == hi[k]));
            if same && phi[axis] + 1 == lo[axis] {
                phi[axis] = hi[axis];
                continue;
            }
        }
        out.push((lo, hi));
    }
    out
}

fn norm_biome(s: &str) -> Result<String, String> {
    let s = s.trim();
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_:./-".contains(c)) {
        return Err(format!("Geçersiz biyom kimliği: {s}"));
    }
    Ok(if s.contains(':') { s.to_string() } else { format!("minecraft:{s}") })
}

/// The Biome Painter: biomes live in 4x4x4 cells, so every cell the brush
/// touches is set whole.
pub struct BiomePainter {
    radius: i32,
    shape: Shape,
    biome: String,
    /// Fill Vertically: the whole height of the world.
    vertical: bool,
    /// Visualize Biome (the client draws the real cells; no effect here).
    visualize: bool,
}

impl BiomePainter {
    pub fn new() -> Self {
        // Inferred: the docs name no default biome or radius.
        BiomePainter { radius: 4, shape: Shape::Sphere, biome: "minecraft:plains".to_string(), vertical: false, visualize: true }
    }
}

impl Tool for BiomePainter {
    fn id(&self) -> &'static str {
        "biyom"
    }

    fn name(&self) -> &'static str {
        "Biyom Boyacısı"
    }

    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        match option {
            "yaricap" | "yarıçap" => {
                self.radius = number::<i32>(value, 0, MAX_RADIUS)?;
                Ok(())
            }
            "sekil" | "şekil" => {
                let w = text(value)?;
                self.shape = Shape::parse(w).ok_or_else(|| format!("Bilinmeyen şekil: {w}"))?;
                Ok(())
            }
            "biyom" => {
                self.biome = norm_biome(text(value)?)?;
                Ok(())
            }
            "dikey" => toggle(&mut self.vertical, value),
            "goster" | "göster" => toggle(&mut self.visualize, value),
            _ => Err(format!("Bilinmeyen seçenek: {option}")),
        }
    }

    fn describe(&self) -> Vec<String> {
        vec![
            format!("yaricap: {}", self.radius),
            format!("sekil: {}", self.shape.name()),
            format!("biyom: {}", self.biome),
            format!("dikey: {}", on_off(self.vertical)),
            format!("goster: {}", on_off(self.visualize)),
        ]
    }

    fn apply(&self, _world: &dyn Blocks, input: &Input) -> Outcome {
        if input.stroke.is_empty() {
            return msg("Fırça darbesi yok.");
        }
        let stroke = match capped_stroke(&input.stroke, self.radius, self.shape) {
            Ok(s) => s,
            Err(m) => return msg(&m),
        };
        let min_cell = WORLD_MIN_Y.div_euclid(4);
        let max_cell = WORLD_MAX_Y.div_euclid(4);
        let cells: BTreeSet<Pos> = stroke.iter().map(|p| p.map(|v| v.div_euclid(4))).collect();
        let mut boxes: Vec<(Pos, Pos)> = if self.vertical {
            let columns: BTreeSet<(i32, i32)> = cells.iter().map(|c| (c[0], c[2])).collect();
            columns.into_iter().map(|(x, z)| ([x, min_cell, z], [x, max_cell, z])).collect()
        } else {
            cells.iter().filter(|c| c[1] >= min_cell && c[1] <= max_cell).map(|c| (*c, *c)).collect()
        };
        for axis in [0, 2, 1] {
            boxes = merge_axis(boxes, axis);
        }
        let out: Vec<(Pos, Pos, String)> = boxes
            .into_iter()
            .map(|(lo, hi)| (lo.map(|v| v * 4), hi.map(|v| v * 4 + 3), self.biome.clone()))
            .collect();
        if out.is_empty() {
            return msg("Boyanacak biyom hücresi yok.");
        }
        Outcome::Biomes(out)
    }
}

// ---------------------------------------------------------------------------
// Script Brush
// ---------------------------------------------------------------------------
//
// A small Lua subset, std only. Supported: `x`, `y`, `z`; numbers, strings
// ('..' or ".."), `true`/`false`/`nil`; `+ - * / % ^`, comparisons, `and`,
// `or`, `not`; assignments (`local` is accepted), `if/elseif/else/end`,
// `for i = a, b[, step] do ... end`, `return <block>`; `blocks.name` or a bare
// name for a block; the functions getBlock, getBlockState, setBlock, isSolid,
// getHighestBlockYAt, withBlockProperty, getBlockProperty, isBlockTagged,
// getSimplexNoise, getVoroniEdgeNoise and math.floor/ceil/abs/sqrt/sin/cos/
// min/max; template variables `$once$`, `$int(..)$`, `$float(..)$`,
// `$boolean(..)$`, `$blockState(..)$` (their defaults are used).
// Not supported: function definitions, tables, while/repeat loops, string
// concatenation, findClosestBlockToRGB and getBlockRGB (they stop the script
// with an error).

const STEP_LIMIT: u64 = 40_000_000;

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Id(String),
    Sym(&'static str),
    Eof,
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let c: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        if ch == '-' && c.get(i + 1) == Some(&'-') {
            if c.get(i + 2) == Some(&'[') && c.get(i + 3) == Some(&'[') {
                i += 4;
                while i < c.len() && !(c[i] == ']' && c.get(i + 1) == Some(&']')) {
                    i += 1;
                }
                i += 2;
            } else {
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
            }
            continue;
        }
        if ch.is_ascii_digit() || (ch == '.' && c.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let s = i;
            while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') {
                i += 1;
            }
            let t: String = c[s..i].iter().collect();
            out.push(Tok::Num(t.parse().map_err(|_| format!("Geçersiz sayı: {t}"))?));
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let s = i;
            while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '_') {
                i += 1;
            }
            out.push(Tok::Id(c[s..i].iter().collect()));
            continue;
        }
        if ch == '\'' || ch == '"' {
            let s = i + 1;
            i += 1;
            while i < c.len() && c[i] != ch {
                i += 1;
            }
            if i >= c.len() {
                return Err("Kapanmamış metin.".to_string());
            }
            out.push(Tok::Str(c[s..i].iter().collect()));
            i += 1;
            continue;
        }
        let two: String = c[i..(i + 2).min(c.len())].iter().collect();
        let sym2 = match two.as_str() {
            "==" => Some("=="),
            "~=" => Some("~="),
            "<=" => Some("<="),
            ">=" => Some(">="),
            _ => None,
        };
        if let Some(s) = sym2 {
            out.push(Tok::Sym(s));
            i += 2;
            continue;
        }
        let s: &'static str = match ch {
            '+' => "+",
            '-' => "-",
            '*' => "*",
            '/' => "/",
            '%' => "%",
            '^' => "^",
            '(' => "(",
            ')' => ")",
            ',' => ",",
            '=' => "=",
            '<' => "<",
            '>' => ">",
            '.' => ".",
            ';' => ";",
            _ => return Err(format!("Tanınmayan karakter: {ch}")),
        };
        out.push(Tok::Sym(s));
        i += 1;
    }
    out.push(Tok::Eof);
    Ok(out)
}

#[derive(Clone, Debug)]
enum Expr {
    Num(f64),
    Str(String),
    Bool(bool),
    Nil,
    Var(String),
    /// `blocks.name`.
    Field(String),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Bin(&'static str, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Clone, Debug)]
enum Stmt {
    Assign(String, Expr),
    If(Vec<(Expr, Vec<Stmt>)>, Vec<Stmt>),
    For(String, Expr, Expr, Option<Expr>, Vec<Stmt>),
    Return(Option<Expr>),
    Call(String, Vec<Expr>),
}

const KEYWORDS: &[&str] = &[
    "and", "or", "not", "if", "then", "else", "elseif", "end", "for", "do", "return", "local", "function", "while", "repeat",
    "until", "true", "false", "nil", "in", "break",
];

struct Parser {
    t: Vec<Tok>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.t[self.i]
    }

    fn next(&mut self) -> Tok {
        let t = self.t[self.i].clone();
        if self.i + 1 < self.t.len() {
            self.i += 1;
        }
        t
    }

    fn is_sym(&self, s: &str) -> bool {
        matches!(self.peek(), Tok::Sym(x) if *x == s)
    }

    fn sym(&mut self, s: &str) -> bool {
        if self.is_sym(s) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn is_kw(&self, k: &str) -> bool {
        matches!(self.peek(), Tok::Id(x) if x.as_str() == k)
    }

    fn kw(&mut self, k: &str) -> bool {
        if self.is_kw(k) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn expect_sym(&mut self, s: &str) -> Result<(), String> {
        if self.sym(s) {
            Ok(())
        } else {
            Err(format!("'{s}' bekleniyordu, bulundu: {:?}", self.peek()))
        }
    }

    fn expect_kw(&mut self, k: &str) -> Result<(), String> {
        if self.kw(k) {
            Ok(())
        } else {
            Err(format!("'{k}' bekleniyordu, bulundu: {:?}", self.peek()))
        }
    }

    fn ident(&mut self) -> Result<String, String> {
        match self.next() {
            Tok::Id(s) if !KEYWORDS.contains(&s.as_str()) => Ok(s),
            t => Err(format!("Ad bekleniyordu, bulundu: {t:?}")),
        }
    }

    fn at_block_end(&self) -> bool {
        matches!(self.peek(), Tok::Eof) || self.is_kw("end") || self.is_kw("else") || self.is_kw("elseif")
    }

    fn block(&mut self) -> Result<Vec<Stmt>, String> {
        let mut v = Vec::new();
        loop {
            while self.sym(";") {}
            if self.at_block_end() {
                break;
            }
            v.push(self.stmt()?);
        }
        Ok(v)
    }

    fn stmt(&mut self) -> Result<Stmt, String> {
        for k in ["function", "while", "repeat", "break"] {
            if self.is_kw(k) {
                return Err(format!("'{k}' desteklenmiyor."));
            }
        }
        self.kw("local");
        if self.kw("if") {
            let mut arms = Vec::new();
            let mut els = Vec::new();
            let c = self.expr()?;
            self.expect_kw("then")?;
            let b = self.block()?;
            arms.push((c, b));
            loop {
                if self.kw("elseif") {
                    let c = self.expr()?;
                    self.expect_kw("then")?;
                    let b = self.block()?;
                    arms.push((c, b));
                } else if self.kw("else") {
                    els = self.block()?;
                    self.expect_kw("end")?;
                    break;
                } else {
                    self.expect_kw("end")?;
                    break;
                }
            }
            return Ok(Stmt::If(arms, els));
        }
        if self.kw("for") {
            let name = self.ident()?;
            self.expect_sym("=")?;
            let a = self.expr()?;
            self.expect_sym(",")?;
            let b = self.expr()?;
            let step = if self.sym(",") { Some(self.expr()?) } else { None };
            self.expect_kw("do")?;
            let body = self.block()?;
            self.expect_kw("end")?;
            return Ok(Stmt::For(name, a, b, step, body));
        }
        if self.kw("return") {
            if self.at_block_end() || self.is_sym(";") {
                return Ok(Stmt::Return(None));
            }
            return Ok(Stmt::Return(Some(self.expr()?)));
        }
        let name = self.ident()?;
        if self.sym("=") {
            let e = self.expr()?;
            return Ok(Stmt::Assign(name, e));
        }
        if self.sym("(") {
            let args = self.args()?;
            return Ok(Stmt::Call(name, args));
        }
        Err(format!("Beklenmeyen ifade: {name}"))
    }

    /// The arguments after an opening `(`.
    fn args(&mut self) -> Result<Vec<Expr>, String> {
        let mut v = Vec::new();
        if self.sym(")") {
            return Ok(v);
        }
        loop {
            v.push(self.expr()?);
            if self.sym(")") {
                return Ok(v);
            }
            self.expect_sym(",")?;
        }
    }

    fn expr(&mut self) -> Result<Expr, String> {
        self.or()
    }

    fn or(&mut self) -> Result<Expr, String> {
        let mut l = self.and()?;
        while self.kw("or") {
            let r = self.and()?;
            l = Expr::Bin("or", Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn and(&mut self) -> Result<Expr, String> {
        let mut l = self.cmp()?;
        while self.kw("and") {
            let r = self.cmp()?;
            l = Expr::Bin("and", Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn bin(&mut self, ops: &[&'static str], next: fn(&mut Parser) -> Result<Expr, String>) -> Result<Expr, String> {
        let mut l = next(self)?;
        loop {
            let mut found = None;
            for op in ops {
                if self.sym(op) {
                    found = Some(*op);
                    break;
                }
            }
            match found {
                Some(op) => {
                    let r = next(self)?;
                    l = Expr::Bin(op, Box::new(l), Box::new(r));
                }
                None => return Ok(l),
            }
        }
    }

    fn cmp(&mut self) -> Result<Expr, String> {
        self.bin(&["==", "~=", "<=", ">=", "<", ">"], Parser::add)
    }

    fn add(&mut self) -> Result<Expr, String> {
        self.bin(&["+", "-"], Parser::mul)
    }

    fn mul(&mut self) -> Result<Expr, String> {
        self.bin(&["*", "/", "%"], Parser::unary)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.kw("not") {
            return Ok(Expr::Not(Box::new(self.unary()?)));
        }
        if self.sym("-") {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        let base = self.primary()?;
        if self.sym("^") {
            let e = self.unary()?;
            return Ok(Expr::Bin("^", Box::new(base), Box::new(e)));
        }
        Ok(base)
    }

    fn primary(&mut self) -> Result<Expr, String> {
        match self.next() {
            Tok::Num(n) => Ok(Expr::Num(n)),
            Tok::Str(s) => Ok(Expr::Str(s)),
            Tok::Sym("(") => {
                let e = self.expr()?;
                self.expect_sym(")")?;
                Ok(e)
            }
            Tok::Id(name) => match name.as_str() {
                "true" => Ok(Expr::Bool(true)),
                "false" => Ok(Expr::Bool(false)),
                "nil" => Ok(Expr::Nil),
                "blocks" if self.is_sym(".") => {
                    self.sym(".");
                    Ok(Expr::Field(self.ident()?))
                }
                "math" if self.is_sym(".") => {
                    self.sym(".");
                    let f = self.ident()?;
                    self.expect_sym("(")?;
                    let args = self.args()?;
                    Ok(Expr::Call(f, args))
                }
                _ => {
                    if KEYWORDS.contains(&name.as_str()) {
                        return Err(format!("Beklenmeyen kelime: {name}"));
                    }
                    if self.sym("(") {
                        let args = self.args()?;
                        Ok(Expr::Call(name, args))
                    } else {
                        Ok(Expr::Var(name))
                    }
                }
            },
            t => Err(format!("Beklenmeyen simge: {t:?}")),
        }
    }
}

/// Replaces the `$...$` template variables by their defaults; reports `$once$`.
fn expand_templates(src: &str) -> Result<(String, bool), String> {
    let mut out = String::new();
    let mut once = false;
    let mut rest = src;
    while let Some(i) = rest.find('$') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let Some(j) = after.find('$') else {
            return Err("Kapanmamış $ şablon değişkeni.".to_string());
        };
        let inner = after[..j].trim();
        rest = &after[j + 1..];
        if inner == "once" {
            once = true;
            continue;
        }
        let Some((name, args)) = inner.split_once('(') else {
            return Err(format!("Geçersiz şablon değişkeni: {inner}"));
        };
        let args = args.strip_suffix(')').ok_or("Şablon değişkeninde ')' eksik.")?;
        let parts: Vec<&str> = args.split(',').map(str::trim).collect();
        let default = parts.get(1).copied().unwrap_or("");
        match name.trim() {
            "int" | "float" | "boolean" | "blockState" => {
                if default.is_empty() {
                    return Err(format!("{name}: varsayılan değer yok."));
                }
                out.push_str(default);
            }
            other => return Err(format!("Bilinmeyen şablon değişkeni: {other}")),
        }
    }
    out.push_str(rest);
    Ok((out, once))
}

struct Program {
    body: Vec<Stmt>,
    once: bool,
}

fn compile(src: &str) -> Result<Program, String> {
    let (text, once) = expand_templates(src)?;
    let mut p = Parser { t: lex(&text)?, i: 0 };
    let body = p.block()?;
    if !matches!(p.peek(), Tok::Eof) {
        return Err(format!("Beklenmeyen simge: {:?}", p.peek()));
    }
    Ok(Program { body, once })
}

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Nil,
    Num(f64),
    Bool(bool),
    Str(String),
    /// A block: `minecraft:wheat` or `minecraft:wheat[age=7]`.
    Block(String),
}

impl Value {
    fn truthy(&self) -> bool {
        !matches!(self, Value::Nil | Value::Bool(false))
    }
}

enum Flow {
    Next,
    Return(Value),
}

fn as_num(v: &Value) -> Result<f64, String> {
    match v {
        Value::Num(n) => Ok(*n),
        other => Err(format!("Sayı bekleniyordu: {other:?}")),
    }
}

fn as_block(v: &Value) -> Result<String, String> {
    match v {
        Value::Block(b) => Ok(b.clone()),
        Value::Str(s) => Ok(norm_block(s)),
        other => Err(format!("Blok bekleniyordu: {other:?}")),
    }
}

fn as_text(v: &Value) -> Result<String, String> {
    match v {
        Value::Str(s) | Value::Block(s) => Ok(s.clone()),
        other => Err(format!("Metin bekleniyordu: {other:?}")),
    }
}

fn arg_num(a: &[Value], i: usize) -> Result<f64, String> {
    a.get(i).ok_or_else(|| "Eksik argüman.".to_string()).and_then(as_num)
}

fn to_coord(f: f64) -> Result<i32, String> {
    if !f.is_finite() || f.abs() > 3.0e7 {
        Err("Konum aralık dışı.".to_string())
    } else {
        Ok(f.floor() as i32)
    }
}

fn pos3(a: &[Value]) -> Result<Pos, String> {
    Ok([to_coord(arg_num(a, 0)?)?, to_coord(arg_num(a, 1)?)?, to_coord(arg_num(a, 2)?)?])
}

/// Whether a block name is solid, by name.
// Inferred: a block value taken out of the world carries no collision data,
// so this list of non-solid names stands in for it.
fn solid_guess(id: &str) -> bool {
    let n = local_name(id);
    if matches!(
        n,
        "air" | "cave_air" | "void_air" | "water" | "lava" | "fire" | "short_grass" | "grass" | "tall_grass" | "fern"
            | "large_fern" | "vine" | "torch" | "wall_torch" | "sugar_cane" | "wheat" | "light" | "poppy" | "dandelion"
    ) {
        return false;
    }
    !(n.ends_with("_sapling") || n.ends_with("_tulip") || n.ends_with("_button") || n.ends_with("_sign") || n.ends_with("_orchid"))
}

// Inferred: a tag (`fences`, `logs`) matches blocks whose name ends in the
// tag word, singular or plural; the game's real tag lists are not reachable.
fn is_tagged(id: &str, tag: &str) -> bool {
    let tag = local_name(tag.trim_start_matches('#'));
    let singular = tag.strip_suffix('s').unwrap_or(tag);
    let n = local_name(id);
    [tag, singular].iter().any(|t| n == *t || n.ends_with(&format!("_{t}")))
}

fn set_prop(state: &str, key: &str, val: &str) -> String {
    let mut parts: Vec<String> = state.split(',').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
    let mut found = false;
    for p in parts.iter_mut() {
        if p.split('=').next() == Some(key) {
            *p = format!("{key}={val}");
            found = true;
        }
    }
    if !found {
        parts.push(format!("{key}={val}"));
    }
    parts.join(",")
}

fn bin_op(op: &str, l: Value, r: Value) -> Result<Value, String> {
    match op {
        "==" => Ok(Value::Bool(l == r)),
        "~=" => Ok(Value::Bool(l != r)),
        "<" | "<=" | ">" | ">=" => {
            let (a, b) = (as_num(&l)?, as_num(&r)?);
            Ok(Value::Bool(match op {
                "<" => a < b,
                "<=" => a <= b,
                ">" => a > b,
                _ => a >= b,
            }))
        }
        "+" | "-" | "*" | "/" | "%" | "^" => {
            let (a, b) = (as_num(&l)?, as_num(&r)?);
            Ok(Value::Num(match op {
                "+" => a + b,
                "-" => a - b,
                "*" => a * b,
                "/" => a / b,
                "%" => a - (a / b).floor() * b,
                _ => a.powf(b),
            }))
        }
        _ => Err(format!("Bilinmeyen işleç: {op}")),
    }
}

/// A running script: reads the world as it was, collects the edits.
struct Run<'a> {
    world: &'a dyn Blocks,
    vars: HashMap<String, Value>,
    edits: BTreeMap<Pos, String>,
    steps: u64,
}

impl<'a> Run<'a> {
    fn spend(&mut self, n: u64) -> Result<(), String> {
        self.steps += n;
        if self.steps > STEP_LIMIT {
            Err("Betik çok uzun sürdü (adım sınırı).".to_string())
        } else {
            Ok(())
        }
    }

    fn block_at(&self, p: Pos) -> Block {
        match self.world.block(p) {
            Some(b) if b.air() => Block { id: AIR.to_string(), state: String::new(), solid: false },
            Some(b) => b,
            None => Block { id: AIR.to_string(), state: String::new(), solid: false },
        }
    }

    fn eval(&mut self, e: &Expr) -> Result<Value, String> {
        self.spend(1)?;
        match e {
            Expr::Num(n) => Ok(Value::Num(*n)),
            Expr::Str(s) => Ok(Value::Str(s.clone())),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Nil => Ok(Value::Nil),
            // A name that is not a variable is a block name.
            Expr::Var(n) => Ok(self.vars.get(n).cloned().unwrap_or_else(|| Value::Block(norm_block(n)))),
            Expr::Field(f) => Ok(Value::Block(norm_block(f))),
            Expr::Not(e) => Ok(Value::Bool(!self.eval(e)?.truthy())),
            Expr::Neg(e) => Ok(Value::Num(-as_num(&self.eval(e)?)?)),
            Expr::Bin("and", l, r) => {
                let a = self.eval(l)?;
                if a.truthy() {
                    self.eval(r)
                } else {
                    Ok(a)
                }
            }
            Expr::Bin("or", l, r) => {
                let a = self.eval(l)?;
                if a.truthy() {
                    Ok(a)
                } else {
                    self.eval(r)
                }
            }
            Expr::Bin(op, l, r) => {
                let a = self.eval(l)?;
                let b = self.eval(r)?;
                bin_op(op, a, b)
            }
            Expr::Call(name, args) => {
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(self.eval(a)?);
                }
                self.call(name, vals)
            }
        }
    }

    fn place(&mut self, p: Pos, block: String) {
        self.edits.insert(p, block);
    }

    fn call(&mut self, name: &str, a: Vec<Value>) -> Result<Value, String> {
        match name {
            "getBlock" => {
                let p = pos3(&a)?;
                Ok(Value::Block(self.block_at(p).id))
            }
            "getBlockState" => {
                let p = pos3(&a)?;
                Ok(Value::Block(block_string(&self.block_at(p))))
            }
            "setBlock" => {
                let p = pos3(&a)?;
                let b = as_block(a.get(3).ok_or("setBlock: blok eksik.")?)?;
                self.place(p, b);
                Ok(Value::Nil)
            }
            "isSolid" => {
                let b = as_block(a.first().ok_or("isSolid: blok eksik.")?)?;
                Ok(Value::Bool(solid_guess(split_block(&b).0)))
            }
            "getHighestBlockYAt" => {
                let x = to_coord(arg_num(&a, 0)?)?;
                let z = to_coord(arg_num(&a, 1)?)?;
                self.spend(64)?;
                for y in (WORLD_MIN_Y..=WORLD_MAX_Y).rev() {
                    if self.world.block([x, y, z]).is_some_and(|b| !b.air()) {
                        return Ok(Value::Num(f64::from(y)));
                    }
                }
                Ok(Value::Num(f64::from(WORLD_MIN_Y)))
            }
            "withBlockProperty" => {
                let b = as_block(a.first().ok_or("withBlockProperty: blok eksik.")?)?;
                let prop = as_text(a.get(1).ok_or("withBlockProperty: özellik eksik.")?)?;
                let (k, v) = prop.split_once('=').ok_or("Özellik 'ad=değer' olmalı.")?;
                let (id, state) = split_block(&b);
                Ok(Value::Block(with_state(id, &set_prop(state, k.trim(), v.trim()))))
            }
            "getBlockProperty" => {
                let b = as_block(a.first().ok_or("getBlockProperty: blok eksik.")?)?;
                let key = as_text(a.get(1).ok_or("getBlockProperty: özellik eksik.")?)?;
                let (_, state) = split_block(&b);
                for part in state.split(',') {
                    if let Some((k, v)) = part.split_once('=') {
                        if k == key {
                            return Ok(Value::Str(v.to_string()));
                        }
                    }
                }
                Ok(Value::Nil)
            }
            "isBlockTagged" => {
                let b = as_block(a.first().ok_or("isBlockTagged: blok eksik.")?)?;
                let tag = as_text(a.get(1).ok_or("isBlockTagged: etiket eksik.")?)?;
                Ok(Value::Bool(is_tagged(split_block(&b).0, &tag)))
            }
            "getSimplexNoise" => {
                let p = [arg_num(&a, 0)?, arg_num(&a, 1)?, arg_num(&a, 2)?];
                // Inferred: the docs randomise a missing seed; a fixed 0 keeps
                // a stroke repeatable. Features are 8 blocks wide.
                let seed = a.get(3).map(as_num).transpose()?.unwrap_or(0.0) as i64 as u64;
                Ok(Value::Num((noise3(p, 8.0, seed) + 1.0) / 2.0))
            }
            "getVoroniEdgeNoise" => {
                let p = [to_coord(arg_num(&a, 0)?)?, to_coord(arg_num(&a, 1)?)?, to_coord(arg_num(&a, 2)?)?];
                let seed = a.get(3).map(as_num).transpose()?.unwrap_or(0.0) as i64 as u64;
                let o = NoiseOpts { kind: NoiseKind::VoronoiEdges, seed, ..NoiseOpts::default() };
                Ok(Value::Num(noise_value(&o, p)))
            }
            "floor" => Ok(Value::Num(arg_num(&a, 0)?.floor())),
            "ceil" => Ok(Value::Num(arg_num(&a, 0)?.ceil())),
            "abs" => Ok(Value::Num(arg_num(&a, 0)?.abs())),
            "sqrt" => Ok(Value::Num(arg_num(&a, 0)?.sqrt())),
            "sin" => Ok(Value::Num(arg_num(&a, 0)?.sin())),
            "cos" => Ok(Value::Num(arg_num(&a, 0)?.cos())),
            "min" => Ok(Value::Num(arg_num(&a, 0)?.min(arg_num(&a, 1)?))),
            "max" => Ok(Value::Num(arg_num(&a, 0)?.max(arg_num(&a, 1)?))),
            "findClosestBlockToRGB" | "getBlockRGB" => Err(format!("{name} desteklenmiyor.")),
            _ => Err(format!("Bilinmeyen işlev: {name}")),
        }
    }

    fn exec(&mut self, body: &[Stmt]) -> Result<Flow, String> {
        for s in body {
            self.spend(1)?;
            match s {
                Stmt::Assign(n, e) => {
                    let v = self.eval(e)?;
                    self.vars.insert(n.clone(), v);
                }
                Stmt::If(arms, els) => {
                    let mut taken = false;
                    for (c, b) in arms {
                        if self.eval(c)?.truthy() {
                            taken = true;
                            if let Flow::Return(v) = self.exec(b)? {
                                return Ok(Flow::Return(v));
                            }
                            break;
                        }
                    }
                    if !taken {
                        if let Flow::Return(v) = self.exec(els)? {
                            return Ok(Flow::Return(v));
                        }
                    }
                }
                Stmt::For(name, a, b, step, body) => {
                    let start = as_num(&self.eval(a)?)?;
                    let end = as_num(&self.eval(b)?)?;
                    let step = match step {
                        Some(e) => as_num(&self.eval(e)?)?,
                        None => 1.0,
                    };
                    if step == 0.0 {
                        return Err("for adımı sıfır olamaz.".to_string());
                    }
                    let mut i = start;
                    while (step > 0.0 && i <= end) || (step < 0.0 && i >= end) {
                        self.spend(1)?;
                        self.vars.insert(name.clone(), Value::Num(i));
                        if let Flow::Return(v) = self.exec(body)? {
                            return Ok(Flow::Return(v));
                        }
                        i += step;
                    }
                }
                Stmt::Return(e) => {
                    let v = match e {
                        Some(e) => self.eval(e)?,
                        None => Value::Nil,
                    };
                    return Ok(Flow::Return(v));
                }
                Stmt::Call(n, args) => {
                    let mut vals = Vec::with_capacity(args.len());
                    for a in args {
                        vals.push(self.eval(a)?);
                    }
                    self.call(n, vals)?;
                }
            }
        }
        Ok(Flow::Next)
    }

    /// Runs the script for one target block.
    fn target(&mut self, body: &[Stmt], p: Pos) -> Result<(), String> {
        self.vars.clear();
        for (k, name) in ["x", "y", "z"].iter().enumerate() {
            self.vars.insert((*name).to_string(), Value::Num(f64::from(p[k])));
        }
        if let Flow::Return(v) = self.exec(body)? {
            match v {
                Value::Nil => {}
                Value::Block(b) => self.place(p, b),
                Value::Str(s) => self.place(p, norm_block(&s)),
                other => return Err(format!("return bir blok olmalı: {other:?}")),
            }
        }
        Ok(())
    }
}

/// The Script Brush: runs a script for every target block of the stroke.
pub struct ScriptBrush {
    radius: i32,
    shape: Shape,
    source: String,
}

impl ScriptBrush {
    pub fn new() -> Self {
        ScriptBrush { radius: 4, shape: Shape::Sphere, source: String::new() }
    }
}

impl Tool for ScriptBrush {
    fn id(&self) -> &'static str {
        "betik"
    }

    fn name(&self) -> &'static str {
        "Betik Fırçası"
    }

    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        match option {
            "yaricap" | "yarıçap" => {
                self.radius = number::<i32>(value, 0, MAX_RADIUS)?;
                Ok(())
            }
            "sekil" | "şekil" => {
                let w = text(value)?;
                self.shape = Shape::parse(w).ok_or_else(|| format!("Bilinmeyen şekil: {w}"))?;
                Ok(())
            }
            "betik" => {
                match value.map(str::trim).filter(|v| !v.is_empty()) {
                    None => self.source.clear(),
                    Some(src) => {
                        compile(src).map_err(|e| format!("Betik hatası: {e}"))?;
                        self.source = src.to_string();
                    }
                }
                Ok(())
            }
            _ => Err(format!("Bilinmeyen seçenek: {option}")),
        }
    }

    fn describe(&self) -> Vec<String> {
        vec![
            format!("yaricap: {}", self.radius),
            format!("sekil: {}", self.shape.name()),
            format!("betik: {}", if self.source.is_empty() { "(boş)" } else { self.source.as_str() }),
        ]
    }

    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        if self.source.trim().is_empty() {
            return msg("Betik boş: /eksen ayar betik <kod>");
        }
        if input.stroke.is_empty() {
            return msg("Fırça darbesi yok.");
        }
        let prog = match compile(&self.source) {
            Ok(p) => p,
            Err(e) => return msg(&format!("Betik hatası: {e}")),
        };
        let positions: Vec<Pos> = if prog.once {
            vec![input.stroke[0]]
        } else {
            match capped_stroke(&input.stroke, self.radius, self.shape) {
                Ok(s) => s.into_iter().collect(),
                Err(m) => return msg(&m),
            }
        };
        let mut run = Run { world, vars: HashMap::new(), edits: BTreeMap::new(), steps: 0 };
        for p in positions {
            if let Err(e) = run.target(&prog.body, p) {
                return msg(&format!("Betik hatası: {e}"));
            }
        }
        let mut out = BTreeMap::new();
        for (p, b) in run.edits {
            let Some(old) = world.block(p) else { continue };
            let both_air = old.air() && is_air_id(split_block(&b).0);
            if !both_air && block_string(&old) != b {
                out.insert(p, b);
            }
        }
        finish(out)
    }
}

// ---------------------------------------------------------------------------
// Gradient Painter
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GradKind {
    Plane,
    Sphere,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lock {
    None,
    Pos1,
    Both,
}

#[derive(Default)]
struct Anchors {
    p1: Option<Pos>,
    p2: Option<Pos>,
}

/// The Gradient Painter: the stroke's first block is pos1, its last is pos2;
/// the palette's blocks, in order, run from one to the other.
pub struct GradientPainter {
    c: Common,
    palette: Palette,
    kind: GradKind,
    interp: Interp,
    lock: Lock,
    clamp: bool,
    state: Mutex<Anchors>,
}

const GRADIENT_SEED: u64 = 0x6772_6164;

impl GradientPainter {
    pub fn new() -> Self {
        GradientPainter {
            c: Common::new(),
            palette: Palette::default(),
            // Inferred: the docs name no default type or interpolation.
            kind: GradKind::Plane,
            interp: Interp::Linear,
            lock: Lock::None,
            clamp: false,
            state: Mutex::new(Anchors::default()),
        }
    }

    fn clear_anchors(&mut self) {
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = Anchors::default();
    }
}

impl Tool for GradientPainter {
    fn id(&self) -> &'static str {
        "gecis"
    }

    fn name(&self) -> &'static str {
        "Geçiş Boyacısı"
    }

    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        if let Some(r) = self.c.set(option, value) {
            return r;
        }
        match option {
            "palet" => {
                self.palette = match value.map(str::trim).filter(|v| !v.is_empty()) {
                    Some(v) => Palette::parse(v)?,
                    None => Palette::default(),
                };
                Ok(())
            }
            "tip" => {
                self.kind = match text(value)? {
                    "duzlem" | "düzlem" | "plane" => GradKind::Plane,
                    "kure" | "küre" | "sphere" => GradKind::Sphere,
                    w => return Err(format!("Bilinmeyen geçiş tipi: {w} (duzlem, kure)")),
                };
                Ok(())
            }
            "interpolasyon" => {
                let w = text(value)?;
                self.interp = Interp::parse(w).ok_or_else(|| format!("Bilinmeyen geçiş türü: {w}"))?;
                Ok(())
            }
            "kilit" => {
                self.lock = match text(value)? {
                    "yok" | "none" => Lock::None,
                    "pos1" | "nokta1" => Lock::Pos1,
                    "ikisi" | "both" => Lock::Both,
                    w => return Err(format!("Bilinmeyen kilit: {w} (yok, pos1, ikisi)")),
                };
                self.clear_anchors();
                Ok(())
            }
            "kenet" => toggle(&mut self.clamp, value),
            _ => Err(format!("Bilinmeyen seçenek: {option}")),
        }
    }

    fn describe(&self) -> Vec<String> {
        let mut v = self.c.describe();
        v.push(format!("palet: {}", self.palette.shown()));
        v.push(format!("tip: {}", if self.kind == GradKind::Plane { "duzlem" } else { "kure" }));
        v.push(format!("interpolasyon: {}", self.interp.name()));
        let lock = match self.lock {
            Lock::None => "yok",
            Lock::Pos1 => "pos1",
            Lock::Both => "ikisi",
        };
        v.push(format!("kilit: {lock}"));
        v.push(format!("kenet: {}", on_off(self.clamp)));
        v
    }

    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        if input.stroke.is_empty() {
            return msg("Fırça darbesi yok.");
        }
        let pal = self.palette.with_active(&input.active);
        let blocks = pal.blocks();
        if blocks.is_empty() {
            return msg("Etkin blok yok.");
        }
        let first = input.stroke[0];
        let last = input.stroke[input.stroke.len() - 1];
        let (p1, p2) = {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            match self.lock {
                Lock::None => (first, last),
                Lock::Pos1 => {
                    let p1 = *st.p1.get_or_insert(first);
                    (p1, last)
                }
                Lock::Both => {
                    if st.p1.is_none() || st.p2.is_none() {
                        st.p1 = Some(first);
                        st.p2 = Some(last);
                    }
                    (st.p1.unwrap_or(first), st.p2.unwrap_or(last))
                }
            }
        };
        let targets = match capped_stroke(&input.stroke, self.c.radius, self.c.shape) {
            Ok(t) => t,
            Err(m) => return msg(&m),
        };
        let d = [0, 1, 2].map(|k| f64::from(p2[k] - p1[k]));
        let dd = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        let edits = paint(world, targets.iter().copied(), &self.c, |p, _| {
            let r = [0, 1, 2].map(|k| f64::from(p[k] - p1[k]));
            let t = match self.kind {
                GradKind::Plane => {
                    if dd == 0.0 {
                        0.0
                    } else {
                        (r[0] * d[0] + r[1] * d[1] + r[2] * d[2]) / dd
                    }
                }
                // pos2 is the radius; a zero radius counts as one block.
                GradKind::Sphere => (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt() / dd.sqrt().max(1.0),
            };
            if self.clamp && t > 1.0 {
                return None;
            }
            Some(gradient_pick(&blocks, t, self.interp, hash_pos(GRADIENT_SEED, p)).to_string())
        });
        finish(edits)
    }
}

/// The painting tools, in the docs' order.
pub fn tools() -> Vec<Box<dyn Tool>> {
    let mut v: Vec<Box<dyn Tool>> = Vec::new();
    v.push(Box::new(Painter::new()));
    v.push(Box::new(NoisePainter::new()));
    v.push(Box::new(BiomePainter::new()));
    v.push(Box::new(ScriptBrush::new()));
    v.push(Box::new(GradientPainter::new()));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    struct World(HashMap<Pos, Block>);

    fn b(id: &str, state: &str, solid: bool) -> Block {
        Block { id: format!("minecraft:{id}"), state: state.into(), solid }
    }

    impl World {
        fn new(set: &[(Pos, Block)]) -> Self {
            World(set.iter().cloned().collect())
        }
    }

    impl Blocks for World {
        /// Air within 40 blocks sideways and the build height, nothing beyond.
        fn block(&self, at: Pos) -> Option<Block> {
            if let Some(x) = self.0.get(&at) {
                return Some(x.clone());
            }
            if at[0].abs() <= 40 && at[2].abs() <= 40 && (-64..=320).contains(&at[1]) {
                Some(b("air", "", false))
            } else {
                None
            }
        }
    }

    fn cube(r: i32, block: &Block) -> Vec<(Pos, Block)> {
        let mut v = Vec::new();
        for x in -r..=r {
            for y in -r..=r {
                for z in -r..=r {
                    v.push(([x, y, z], block.clone()));
                }
            }
        }
        v
    }

    fn input(stroke: &[Pos], active: &str) -> Input {
        Input { stroke: stroke.to_vec(), active: active.to_string(), ..Default::default() }
    }

    fn edits(o: Outcome) -> BTreeMap<Pos, String> {
        match o {
            Outcome::Edit(m) => m,
            other => panic!("expected an edit, got {other:?}"),
        }
    }

    fn stone() -> Block {
        b("stone", "", true)
    }

    #[test]
    fn tool_list_is_in_docs_order() {
        let ids: Vec<&str> = tools().iter().map(|t| t.id()).collect();
        assert_eq!(ids, vec!["boyaci", "gurultu", "biyom", "betik", "gecis"]);
    }

    #[test]
    fn palette_parsing() {
        let p = Palette::parse("stone:3, minecraft:andesite:1,oak_stairs[facing=east,half=top]").unwrap();
        assert_eq!(p.entries.len(), 3);
        assert_eq!(p.entries[0], ("minecraft:stone".to_string(), 3.0));
        assert_eq!(p.entries[1], ("minecraft:andesite".to_string(), 1.0));
        assert_eq!(p.entries[2].0, "minecraft:oak_stairs[facing=east,half=top]");
        assert!(Palette::parse("stone:-2").is_err());
        assert_eq!(p.pick(0.0), "minecraft:stone");
        assert_eq!(p.pick(0.99), "minecraft:oak_stairs[facing=east,half=top]");
    }

    #[test]
    fn painter_only_paints_solid_blocks_and_never_air() {
        let w = World::new(&cube(2, &stone()));
        let mut t = Painter::new();
        t.set("yaricap", Some("3")).unwrap();
        t.set("sekil", Some("kup")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], "minecraft:dirt")));
        // The 7-cube brush covers the 5-cube of stone and a shell of air.
        assert_eq!(e.len(), 125);
        assert!(e.values().all(|v| v == "minecraft:dirt"));
        assert!(e.keys().all(|p| p.iter().all(|c| c.abs() <= 2)));
        // Only air under the brush: nothing to paint.
        let empty = World::new(&[]);
        assert!(matches!(t.apply(&empty, &input(&[[0, 0, 0]], "minecraft:dirt")), Outcome::Message(_)));
        // Painting a block with itself changes nothing.
        assert!(matches!(t.apply(&w, &input(&[[0, 0, 0]], "minecraft:stone")), Outcome::Message(_)));
    }

    #[test]
    fn painter_palette_is_weighted() {
        let w = World::new(&cube(6, &b("dirt", "", true)));
        let mut t = Painter::new();
        t.set("yaricap", Some("5")).unwrap();
        t.set("palet", Some("minecraft:stone:3,minecraft:andesite:1")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], "minecraft:glass")));
        let stone_n = e.values().filter(|v| *v == "minecraft:stone").count();
        let andesite_n = e.values().filter(|v| *v == "minecraft:andesite").count();
        assert_eq!(stone_n + andesite_n, e.len());
        assert!(andesite_n > 60 && stone_n > 2 * andesite_n, "{stone_n} / {andesite_n}");
        // The same stroke gives the same result.
        assert_eq!(e, edits(t.apply(&w, &input(&[[0, 0, 0]], "minecraft:glass"))));
    }

    #[test]
    fn painter_surface_mask() {
        let w = World::new(&cube(2, &stone()));
        let mut t = Painter::new();
        t.set("yaricap", Some("4")).unwrap();
        t.set("sekil", Some("kup")).unwrap();
        t.set("yuzey", None).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], "minecraft:dirt")));
        assert_eq!(e.len(), 98);
        assert!(!e.contains_key(&[0, 0, 0]));
    }

    #[test]
    fn painter_type_replace_and_copy_properties() {
        let w = World::new(&[
            ([0, 0, 0], b("oak_stairs", "facing=east,half=bottom", true)),
            ([1, 0, 0], b("oak_planks", "", true)),
            ([2, 0, 0], b("oak_slab", "type=bottom", true)),
        ]);
        let mut t = Painter::new();
        t.set("yaricap", Some("3")).unwrap();
        t.set("sekil", Some("kup")).unwrap();
        t.set("palet", Some("minecraft:stone_stairs")).unwrap();
        t.set("tur_degistir", Some("ac")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], "minecraft:dirt")));
        assert_eq!(e.len(), 1);
        assert_eq!(e[&[0, 0, 0]], "minecraft:stone_stairs[facing=east,half=bottom]");

        let mut c = Painter::new();
        c.set("yaricap", Some("3")).unwrap();
        c.set("sekil", Some("kup")).unwrap();
        c.set("palet", Some("minecraft:birch_stairs")).unwrap();
        c.set("ozellik", Some("ac")).unwrap();
        let e = edits(c.apply(&w, &input(&[[0, 0, 0]], "minecraft:dirt")));
        assert_eq!(e[&[0, 0, 0]], "minecraft:birch_stairs[facing=east,half=bottom]");
        assert_eq!(e[&[1, 0, 0]], "minecraft:birch_stairs");
    }

    #[test]
    fn painter_gradient_mode_and_merge() {
        let mut set = cube(6, &stone());
        set.push(([1, 0, 0], b("black_concrete", "", true)));
        let w = World::new(&set);
        let mut t = Painter::new();
        t.set("yaricap", Some("5")).unwrap();
        t.set("mod", Some("gecis")).unwrap();
        t.set("interpolasyon", Some("yakin")).unwrap();
        t.set("palet", Some("minecraft:white_concrete,minecraft:black_concrete")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], "minecraft:glass")));
        assert_eq!(e[&[0, 0, 0]], "minecraft:white_concrete");
        assert_eq!(e[&[5, 0, 0]], "minecraft:black_concrete");
        // Without merge the black block near the centre is overpainted.
        assert_eq!(e[&[1, 0, 0]], "minecraft:white_concrete");
        t.set("birlestir", Some("ac")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], "minecraft:glass")));
        assert!(!e.contains_key(&[1, 0, 0]));
        assert!(t.set("mod", Some("doseme")).is_err());
    }

    #[test]
    fn noise_values_stay_in_range() {
        for kind in [
            NoiseKind::Simplex,
            NoiseKind::Worley,
            NoiseKind::VoronoiEdges,
            NoiseKind::Metaball,
            NoiseKind::White,
            NoiseKind::Splatter,
        ] {
            let o = NoiseOpts { kind, octaves: 3, weights: [1.0, 0.5, 0.25], ..NoiseOpts::default() };
            for i in 0..300 {
                let v = noise_value(&o, [i * 7 - 500, i % 13 - 6, i * 3]);
                assert!((0.0..=1.0).contains(&v), "{kind:?} gave {v}");
            }
            assert_eq!(noise_value(&o, [3, 4, 5]), noise_value(&o, [3, 4, 5]));
        }
    }

    #[test]
    fn noise_painter_two_d_is_constant_over_a_column() {
        let w = World::new(&cube(6, &stone()));
        let mut t = NoisePainter::new();
        t.set("uc_boyut", Some("kapat")).unwrap();
        t.set("olcek", Some("3")).unwrap();
        t.set("yaricap", Some("5")).unwrap();
        t.set("sekil", Some("kup")).unwrap();
        t.set("palet", Some("minecraft:andesite,minecraft:diorite")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], "minecraft:stone")));
        assert_eq!(e.len(), 11 * 11 * 11);
        for x in -5..=5 {
            for z in -5..=5 {
                let top = &e[&[x, 0, z]];
                assert!((-5..=5).all(|y| &e[&[x, y, z]] == top));
            }
        }
        let kinds: BTreeSet<&String> = e.values().collect();
        assert_eq!(kinds.len(), 2);
    }

    #[test]
    fn noise_painter_percentages_hold() {
        let w = World::new(&cube(10, &stone()));
        let mut t = NoisePainter::new();
        t.set("olcek", Some("4")).unwrap();
        t.set("yaricap", Some("9")).unwrap();
        t.set("palet", Some("minecraft:andesite,minecraft:diorite")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], "minecraft:stone")));
        let share = e.values().filter(|v| *v == "minecraft:andesite").count() as f64 / e.len() as f64;
        assert!((0.2..0.8).contains(&share), "andesite share {share}");
        assert_eq!(e, edits(t.apply(&w, &input(&[[0, 0, 0]], "minecraft:stone"))));
    }

    #[test]
    fn noise_painter_threshold_mode() {
        let w = World::new(&cube(3, &stone()));
        let mut t = NoisePainter::new();
        t.set("yaricap", Some("2")).unwrap();
        t.set("sekil", Some("kup")).unwrap();
        t.set("dagitim", Some("esik")).unwrap();
        t.set("palet", Some("minecraft:andesite:1.0")).unwrap();
        assert_eq!(edits(t.apply(&w, &input(&[[0, 0, 0]], "minecraft:stone"))).len(), 125);
        // Nothing is at or below noise 0: the blocks stay.
        t.set("palet", Some("minecraft:andesite:0")).unwrap();
        assert!(matches!(t.apply(&w, &input(&[[0, 0, 0]], "minecraft:stone")), Outcome::Message(_)));
        assert!(t.set("gurultu", Some("yok boyle")).is_err());
        assert!(t.set("oktav", Some("99")).is_err());
    }

    fn boxes(o: Outcome) -> Vec<(Pos, Pos, String)> {
        match o {
            Outcome::Biomes(v) => v,
            other => panic!("expected biomes, got {other:?}"),
        }
    }

    #[test]
    fn biome_cells_are_four_blocks() {
        let w = World::new(&[]);
        let mut t = BiomePainter::new();
        t.set("yaricap", Some("0")).unwrap();
        t.set("biyom", Some("desert")).unwrap();
        let v = boxes(t.apply(&w, &input(&[[5, 70, 5]], "")));
        assert_eq!(v, vec![([4, 68, 4], [7, 71, 7], "minecraft:desert".to_string())]);
        // Negative coordinates round down.
        let v = boxes(t.apply(&w, &input(&[[-1, -1, -1]], "")));
        assert_eq!(v[0].0, [-4, -4, -4]);
        assert_eq!(v[0].1, [-1, -1, -1]);
        // Two neighbouring cells merge into one box.
        let v = boxes(t.apply(&w, &input(&[[0, 0, 0], [4, 0, 0]], "")));
        assert_eq!(v.len(), 1);
        assert_eq!((v[0].0, v[0].1), ([0, 0, 0], [7, 3, 3]));
        // Fill vertically spans the whole height.
        t.set("dikey", None).unwrap();
        let v = boxes(t.apply(&w, &input(&[[5, 70, 5]], "")));
        assert_eq!((v[0].0, v[0].1), ([4, -64, 4], [7, 319, 7]));
        assert!(t.set("biyom", Some("Bad Biome")).is_err());
    }

    #[test]
    fn biome_brush_merges_into_few_boxes() {
        let w = World::new(&[]);
        let mut t = BiomePainter::new();
        t.set("yaricap", Some("6")).unwrap();
        t.set("sekil", Some("kup")).unwrap();
        let v = boxes(t.apply(&w, &input(&[[2, 2, 2]], "")));
        // A cube covers a block of whole cells: one box.
        assert_eq!(v.len(), 1);
        assert_eq!((v[0].0, v[0].1), ([-4, -4, -4], [11, 11, 11]));
    }

    fn script(code: &str, r: i32) -> ScriptBrush {
        let mut t = ScriptBrush::new();
        t.set("yaricap", Some(&r.to_string())).unwrap();
        t.set("betik", Some(code)).unwrap();
        t
    }

    #[test]
    fn script_condition_and_return() {
        let w = World::new(&cube(2, &stone()));
        let t = script("if y < 100 then return dirt end", 1);
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], "")));
        assert_eq!(e.len(), 19);
        assert!(e.values().all(|v| v == "minecraft:dirt"));
        let t = script("if y > 100 then return dirt end", 1);
        assert!(matches!(t.apply(&w, &input(&[[0, 0, 0]], "")), Outcome::Message(_)));
    }

    #[test]
    fn script_once_and_loops_and_templates() {
        let w = World::new(&[]);
        let t = script("$once$\nsetBlock(x,y,z,blocks.pink_wool)", 3);
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], "")));
        assert_eq!(e.len(), 1);
        assert_eq!(e[&[0, 0, 0]], "minecraft:pink_wool");
        let t = script("$once$\nheight=$int(Height,5,1,10)$\nfor yo=1, height do setBlock(x,y+yo,z,blocks.pink_wool) end", 3);
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], "")));
        assert_eq!(e.keys().copied().collect::<Vec<_>>(), (1..=5).map(|y| [0, y, 0]).collect::<Vec<_>>());
    }

    #[test]
    fn script_carpet_on_ground() {
        let mut set = Vec::new();
        for x in -3..=3 {
            for z in -3..=3 {
                set.push(([x, 0, z], stone()));
            }
        }
        let w = World::new(&set);
        let code = "carpet = blocks.pink_carpet\n\
                    if getBlock(x,y-1,z)~=blocks.air and getBlock(x,y,z)==blocks.air then\n\
                      setBlock(x,y,z,carpet)\n\
                    end";
        let mut t = script(code, 2);
        t.set("sekil", Some("kup")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 1, 0]], "")));
        assert_eq!(e.len(), 25);
        assert!(e.keys().all(|p| p[1] == 1));
    }

    #[test]
    fn script_properties_height_and_noise() {
        let mut w = World::new(&[([0, 0, 0], b("wheat", "age=7", false))]);
        let code = "aged = withBlockProperty(blocks.wheat,'age=7')\n\
                    if getBlockState(x,y,z)==aged then return withBlockProperty(blocks.wheat,'age=0') end";
        let t = script(code, 0);
        let e = edits(t.apply(&w, &input(&[[0, 0, 0]], "")));
        assert_eq!(e[&[0, 0, 0]], "minecraft:wheat[age=0]");
        let t = script("if getBlockProperty(getBlockState(x,y,z),'age')=='7' then return dirt end", 0);
        assert_eq!(edits(t.apply(&w, &input(&[[0, 0, 0]], ""))).len(), 1);

        w.0.insert([0, 0, 0], stone());
        let t = script("if getHighestBlockYAt(x,z)==0 then return snow_block end", 0);
        assert_eq!(edits(t.apply(&w, &input(&[[0, 0, 0]], "")))[&[0, 0, 0]], "minecraft:snow_block");
        let t = script("if getSimplexNoise(x,y,z,7) >= 0 and getSimplexNoise(x,y,z,7) <= 1 then return dirt end", 0);
        assert_eq!(edits(t.apply(&w, &input(&[[0, 0, 0]], ""))).len(), 1);
    }

    #[test]
    fn script_errors_are_messages() {
        let mut t = ScriptBrush::new();
        assert!(t.set("betik", Some("function f() end")).is_err());
        assert!(t.set("betik", Some("if x then")).is_err());
        let w = World::new(&cube(1, &stone()));
        let t = script("return getBlockRGB(getBlock(x,y,z))", 0);
        match t.apply(&w, &input(&[[0, 0, 0]], "")) {
            Outcome::Message(m) => assert!(m[0].contains("desteklenmiyor"), "{m:?}"),
            other => panic!("{other:?}"),
        }
        // An endless loop is cut off.
        let t = script("for i = 1, 1000000000 do a = i end", 0);
        assert!(matches!(t.apply(&w, &input(&[[0, 0, 0]], "")), Outcome::Message(_)));
        assert!(matches!(ScriptBrush::new().apply(&w, &input(&[[0, 0, 0]], "")), Outcome::Message(_)));
    }

    fn gradient_tool() -> GradientPainter {
        let mut t = GradientPainter::new();
        t.set("yaricap", Some("1")).unwrap();
        t.set("interpolasyon", Some("yakin")).unwrap();
        t.set("palet", Some("minecraft:white_concrete,minecraft:gray_concrete,minecraft:black_concrete")).unwrap();
        t
    }

    #[test]
    fn gradient_plane_and_clamp() {
        let w = World::new(&cube(8, &stone()));
        let line: Vec<Pos> = (-5..=5).map(|x| [x, 0, 0]).collect();
        let mut t = gradient_tool();
        let e = edits(t.apply(&w, &input(&line, "")));
        assert_eq!(e[&[-5, 0, 0]], "minecraft:white_concrete");
        assert_eq!(e[&[0, 0, 0]], "minecraft:gray_concrete");
        assert_eq!(e[&[5, 0, 0]], "minecraft:black_concrete");
        // Past pos2 the last block carries on ...
        assert_eq!(e[&[6, 0, 0]], "minecraft:black_concrete");
        // ... unless the gradient is clamped to pos2.
        t.set("kenet", None).unwrap();
        let e = edits(t.apply(&w, &input(&line, "")));
        assert!(!e.contains_key(&[6, 0, 0]));
        assert!(e.contains_key(&[5, 0, 0]));
    }

    #[test]
    fn gradient_sphere() {
        let w = World::new(&cube(8, &stone()));
        let mut t = gradient_tool();
        t.set("yaricap", Some("6")).unwrap();
        t.set("tip", Some("kure")).unwrap();
        t.set("palet", Some("minecraft:white_concrete,minecraft:black_concrete")).unwrap();
        let e = edits(t.apply(&w, &input(&[[0, 0, 0], [6, 0, 0]], "")));
        assert_eq!(e[&[0, 0, 0]], "minecraft:white_concrete");
        assert_eq!(e[&[0, 0, -6]], "minecraft:black_concrete");
    }

    #[test]
    fn gradient_locking() {
        let w = World::new(&cube(8, &stone()));
        let mut t = gradient_tool();
        t.set("yaricap", Some("0")).unwrap();
        t.set("kilit", Some("pos1")).unwrap();
        edits(t.apply(&w, &input(&[[-5, 0, 0], [5, 0, 0]], "")));
        // pos1 stays at -5: x = 0 is the middle of the gradient.
        let e = edits(t.apply(&w, &input(&[[0, 0, 0], [5, 0, 0]], "")));
        assert_eq!(e[&[0, 0, 0]], "minecraft:gray_concrete");

        let mut t = gradient_tool();
        t.set("yaricap", Some("0")).unwrap();
        t.set("kilit", Some("ikisi")).unwrap();
        edits(t.apply(&w, &input(&[[-5, 0, 0], [5, 0, 0]], "")));
        // Both points stay: x = 3 is 80% of the way.
        let e = edits(t.apply(&w, &input(&[[3, 0, 0], [2, 0, 0]], "")));
        assert_eq!(e[&[3, 0, 0]], "minecraft:black_concrete");
        // Unlocking forgets them.
        t.set("kilit", Some("yok")).unwrap();
        let e = edits(t.apply(&w, &input(&[[3, 0, 0], [2, 0, 0]], "")));
        assert_eq!(e[&[3, 0, 0]], "minecraft:white_concrete");
    }

    #[test]
    fn interpolation_modes() {
        let blocks: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        assert_eq!(gradient_pick(&blocks, 0.0, Interp::Nearest, 0.5), "a");
        assert_eq!(gradient_pick(&blocks, 0.5, Interp::Nearest, 0.5), "b");
        assert_eq!(gradient_pick(&blocks, 1.0, Interp::Nearest, 0.5), "c");
        // Linear dithers between neighbours.
        assert_eq!(gradient_pick(&blocks, 0.25, Interp::Linear, 0.9), "a");
        assert_eq!(gradient_pick(&blocks, 0.25, Interp::Linear, 0.1), "b");
        // Bezier eases: a quarter in is less than a quarter of the way.
        assert_eq!(gradient_pick(&blocks, 0.25, Interp::Bezier, 0.4), "a");
    }
}
