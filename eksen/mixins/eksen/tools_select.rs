// Eksen — 16.10.1 (mihehk): the selection and utility tools: Box Select,
// Freehand Select, Lasso Select, Ruler and Annotation. Written from scratch
// after the behaviour Axiom's documentation describes (axiomdocs.moulberry.com,
// "Selection Tools", "Utility Tools"); no code, names or images of Axiom are
// used. Magic Select lives in `eksen_magic`.
//
// A selection tool answers with `Outcome::Select`: the new area only. The
// editor joins it to the current selection by the boolean operation (add,
// subtract, replace, intersect; `eksen_magic::Op`), so no tool here does that.
// A selection past `SELECT_CAP` blocks (or work past a fixed budget) is never
// cut off silently: the tool answers with a chat line saying so instead.
use std::collections::BTreeSet;
use std::sync::{Mutex, MutexGuard};

use super::eksen_core::*;

/// The most blocks one selection may hold.
pub const SELECT_CAP: usize = 1_000_000;
/// The most brush cells / lasso cells a tool tests for one stroke.
const WORK_CAP: i64 = 20_000_000;
/// The most polygon edge tests a lasso does (columns times vertices).
const POLY_WORK_CAP: i64 = 100_000_000;
/// The most characters of one annotation's text.
pub const TEXT_MAX: usize = 256;
/// The most annotations the store keeps.
pub const ANNOTATION_MAX: usize = 10_000;

fn unknown(option: &str) -> String {
    format!("Bilinmeyen seçenek: {option}")
}

fn msg(line: impl Into<String>) -> Outcome {
    Outcome::Message(vec![line.into()])
}

/// Parses "x,y,z" (commas and/or spaces).
fn parse_pos(text: &str) -> Option<Pos> {
    let parts: Vec<&str> = text.split(|c: char| c == ',' || c.is_whitespace()).filter(|s| !s.is_empty()).collect();
    if parts.len() != 3 {
        return None;
    }
    let mut out: Pos = [0; 3];
    for (i, w) in parts.iter().enumerate() {
        out[i] = w.parse::<i32>().ok()?;
    }
    Some(out)
}

fn pos_text(p: Pos) -> String {
    format!("{},{},{}", p[0], p[1], p[2])
}

fn add(a: Pos, d: Pos) -> Pos {
    [a[0].saturating_add(d[0]), a[1].saturating_add(d[1]), a[2].saturating_add(d[2])]
}

// ---------------------------------------------------------------- Box Select

/// Box Select: a cuboid between two corners. The corners are the stroke's
/// first and last hit blocks, or two typed coordinates.
#[derive(Clone, Debug, Default)]
pub struct BoxSelect {
    /// Typed first corner. When both corners are typed they win over the
    /// stroke. (Inferred: with only one typed, the stroke is used.)
    pub pos1: Option<Pos>,
    pub pos2: Option<Pos>,
}

/// The cuboid between corners `a` and `b` (inclusive), or a chat line if it
/// is too big.
fn cuboid(a: Pos, b: Pos) -> Outcome {
    let lo = [0, 1, 2].map(|i| a[i].min(b[i]));
    let hi = [0, 1, 2].map(|i| a[i].max(b[i]));
    let size = [0, 1, 2].map(|i| i64::from(hi[i]) - i64::from(lo[i]) + 1);
    let volume = size[0].saturating_mul(size[1]).saturating_mul(size[2]);
    if volume > SELECT_CAP as i64 {
        return msg(format!("Kutu çok büyük: {volume} blok (en çok {SELECT_CAP})."));
    }
    let mut out = BTreeSet::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                out.insert([x, y, z]);
            }
        }
    }
    Outcome::Select(out)
}

impl Tool for BoxSelect {
    fn id(&self) -> &'static str {
        "kutusecim"
    }
    fn name(&self) -> &'static str {
        "Kutu Seçimi"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        let slot = match option {
            "pos1" => &mut self.pos1,
            "pos2" => &mut self.pos2,
            _ => return Err(unknown(option)),
        };
        match value {
            Some("yok" | "temizle") => *slot = None,
            Some(w) => *slot = Some(parse_pos(w).ok_or_else(|| "Koordinat x,y,z biçiminde olmalı.".to_string())?),
            None => return Err("Koordinat gerekli: x,y,z (ya da yok).".to_string()),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        let show = |p: Option<Pos>| p.map_or_else(|| "yok".to_string(), pos_text);
        vec![format!("pos1: {}", show(self.pos1)), format!("pos2: {}", show(self.pos2))]
    }
    fn apply(&self, _world: &dyn Blocks, input: &Input) -> Outcome {
        if let (Some(a), Some(b)) = (self.pos1, self.pos2) {
            return cuboid(a, b);
        }
        match (input.stroke.first(), input.stroke.last()) {
            (Some(a), Some(b)) => cuboid(*a, *b),
            _ => msg("Kutu için iki köşe gerekli: sürükleyin ya da pos1 ve pos2 girin."),
        }
    }
}

// ------------------------------------------------------------ Freehand Select

/// Freehand Select: the blocks a brush stroke covers.
#[derive(Clone, Debug)]
pub struct Freehand {
    /// Inferred: the docs give no default; 2.
    pub radius: i32,
    /// The brush shape; the documented default is a sphere.
    pub shape: Shape,
}

impl Default for Freehand {
    fn default() -> Self {
        Freehand { radius: 2, shape: Shape::Sphere }
    }
}

/// The brushes along `points`, or why they are too many.
fn painted(points: &[Pos], radius: i32, shape: Shape) -> Result<BTreeSet<Pos>, String> {
    let r = i64::from(radius.max(0));
    let cell = (2 * r + 1) * (2 * r + 1) * (2 * r + 1);
    if (points.len() as i64).saturating_mul(cell) > WORK_CAP {
        return Err(format!("Fırça darbesi çok uzun ya da çok kalın ({} nokta, yarıçap {radius}).", points.len()));
    }
    let mut out = BTreeSet::new();
    for p in points {
        out.extend(brush(*p, radius, shape));
        if out.len() > SELECT_CAP {
            return Err(format!("Seçim çok büyük: en çok {SELECT_CAP} blok."));
        }
    }
    Ok(out)
}

impl Tool for Freehand {
    fn id(&self) -> &'static str {
        "elsecim"
    }
    fn name(&self) -> &'static str {
        "Serbest Seçim"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        match option {
            "yaricap" => self.radius = number::<i32>(value, 0, 32)?,
            "sekil" => {
                self.shape = value.and_then(Shape::parse).ok_or_else(|| "küre, küp ya da sekizyüzlü bekleniyordu.".to_string())?
            }
            _ => return Err(unknown(option)),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        vec![format!("yaricap: {}", self.radius), format!("sekil: {}", self.shape.name())]
    }
    fn apply(&self, _world: &dyn Blocks, input: &Input) -> Outcome {
        if input.stroke.is_empty() {
            return msg("Seçmek için bir bloğa bakarak fırçayı sürükleyin.");
        }
        match painted(&input.stroke, self.radius, self.shape) {
            Ok(set) => Outcome::Select(set),
            Err(e) => msg(e),
        }
    }
}

// ---------------------------------------------------------------- Lasso Select

/// Lasso Select: the stroke's hit blocks make a closed outline which is cast
/// into the world along the view.
///
/// Inferred projection (the docs only say the outline is cast to the blocks
/// in view): the outline is flattened onto the plane perpendicular to the
/// view's dominant axis (the axis `Input.look` points along most), closed from
/// the last point back to the first, and every block whose centre is inside
/// it (or within half a block of its edge) is a column. The column runs along
/// the dominant axis over the stroke's own depth range, widened by `depth`
/// blocks on the far side (away from the viewer). Only solid blocks count,
/// unless `non_solid` is on; air never does.
#[derive(Clone, Debug)]
pub struct Lasso {
    /// Inferred: the docs give a slider but no default; 8 blocks.
    pub depth: i32,
    /// "Include Non-Solid": also blocks such as short grass (default off).
    pub non_solid: bool,
}

impl Default for Lasso {
    fn default() -> Self {
        Lasso { depth: 8, non_solid: false }
    }
}

/// The axis the view points along most.
fn dominant_axis(look: [f64; 3]) -> usize {
    let mut a = 0;
    for i in 1..3 {
        if look[i].abs() > look[a].abs() {
            a = i;
        }
    }
    a
}

/// The smallest and largest coordinate of `points` on `axis`.
fn axis_range(points: &[Pos], axis: usize) -> (i32, i32) {
    let mut lo = i32::MAX;
    let mut hi = i32::MIN;
    for p in points {
        lo = lo.min(p[axis]);
        hi = hi.max(p[axis]);
    }
    (lo, hi)
}

/// Even-odd test of (x, y) against the closed polygon.
fn in_polygon(poly: &[[f64; 2]], x: f64, y: f64) -> bool {
    let n = poly.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > y) != (b[1] > y) && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Whether (x, y) is within `r` of an edge of the closed polygon.
fn near_edge(poly: &[[f64; 2]], x: f64, y: f64, r: f64) -> bool {
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len2 = dx * dx + dy * dy;
        let t = if len2 == 0.0 { 0.0 } else { (((x - a[0]) * dx + (y - a[1]) * dy) / len2).clamp(0.0, 1.0) };
        let (cx, cy) = (a[0] + t * dx, a[1] + t * dy);
        if (x - cx) * (x - cx) + (y - cy) * (y - cy) <= r * r + 1e-9 {
            return true;
        }
    }
    false
}

impl Tool for Lasso {
    fn id(&self) -> &'static str {
        "kement"
    }
    fn name(&self) -> &'static str {
        "Kement Seçimi"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        match option {
            "derinlik" => self.depth = number::<i32>(value, 0, 256)?,
            "katisiz" => toggle(&mut self.non_solid, value)?,
            _ => return Err(unknown(option)),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        vec![format!("derinlik: {}", self.depth), format!("katisiz: {}", on_off(self.non_solid))]
    }
    fn apply(&self, world: &dyn Blocks, input: &Input) -> Outcome {
        let a = dominant_axis(input.look);
        let (u, v) = match a {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        let mut poly: Vec<[f64; 2]> = Vec::new();
        let mut distinct: BTreeSet<(i32, i32)> = BTreeSet::new();
        for p in &input.stroke {
            distinct.insert((p[u], p[v]));
            let q = [f64::from(p[u]) + 0.5, f64::from(p[v]) + 0.5];
            if poly.last() != Some(&q) {
                poly.push(q);
            }
        }
        if distinct.len() < 3 {
            return msg("Kement için çevreyi çizen en az üç farklı nokta gerekli.");
        }
        let (lo_u, hi_u) = axis_range(&input.stroke, u);
        let (lo_v, hi_v) = axis_range(&input.stroke, v);
        let (mut lo_a, mut hi_a) = axis_range(&input.stroke, a);
        // Widen away from the viewer.
        if input.look[a] >= 0.0 {
            hi_a = hi_a.saturating_add(self.depth);
        } else {
            lo_a = lo_a.saturating_sub(self.depth);
        }
        let du = i64::from(hi_u) - i64::from(lo_u) + 1;
        let dv = i64::from(hi_v) - i64::from(lo_v) + 1;
        let da = i64::from(hi_a) - i64::from(lo_a) + 1;
        let columns = du.saturating_mul(dv);
        if columns.saturating_mul(da) > WORK_CAP || columns.saturating_mul(poly.len() as i64) > POLY_WORK_CAP {
            return msg("Kement alanı çok büyük; daha küçük bir çevre çizin ya da derinliği azaltın.");
        }
        let mut found = BTreeSet::new();
        for i in lo_u..=hi_u {
            for j in lo_v..=hi_v {
                let (x, y) = (f64::from(i) + 0.5, f64::from(j) + 0.5);
                if !(in_polygon(&poly, x, y) || near_edge(&poly, x, y, 0.5)) {
                    continue;
                }
                for k in lo_a..=hi_a {
                    let mut p: Pos = [0; 3];
                    p[a] = k;
                    p[u] = i;
                    p[v] = j;
                    let take = match world.block(p) {
                        Some(b) => !b.air() && (b.solid || self.non_solid),
                        None => false,
                    };
                    if take {
                        found.insert(p);
                        if found.len() > SELECT_CAP {
                            return msg(format!("Seçim çok büyük: en çok {SELECT_CAP} blok."));
                        }
                    }
                }
            }
        }
        Outcome::Select(found)
    }
}

// ------------------------------------------------------------------ Ruler Tool

/// Ruler: measures along the stroke's points (the points the player added).
#[derive(Clone, Debug)]
pub struct Ruler {
    /// "Circle" mode: also tell the circle whose radius is the distance
    /// between the two points (default off).
    pub circle: bool,
    /// "Select Center": answer with a selection of the line's centre block
    /// instead of the measurement (default off).
    pub select_center: bool,
    /// "Split": subdivide each line into this many parts (default 1: none).
    pub split: i32,
}

impl Default for Ruler {
    fn default() -> Self {
        Ruler { circle: false, select_center: false, split: 1 }
    }
}

fn diff(a: Pos, b: Pos) -> [i64; 3] {
    [0, 1, 2].map(|i| i64::from(b[i]) - i64::from(a[i]))
}

fn length(d: [i64; 3]) -> f64 {
    let f = d.map(|v| v as f64);
    (f[0] * f[0] + f[1] * f[1] + f[2] * f[2]).sqrt()
}

/// The most lines a ruler answer has.
const RULER_LINES_MAX: usize = 100;

impl Tool for Ruler {
    fn id(&self) -> &'static str {
        "cetvel"
    }
    fn name(&self) -> &'static str {
        "Cetvel"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        match option {
            "daire" => toggle(&mut self.circle, value)?,
            "merkezsec" => toggle(&mut self.select_center, value)?,
            "bol" => self.split = number::<i32>(value, 1, 64)?,
            _ => return Err(unknown(option)),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        vec![
            format!("daire: {}", on_off(self.circle)),
            format!("merkezsec: {}", on_off(self.select_center)),
            format!("bol: {}", self.split),
        ]
    }
    fn apply(&self, _world: &dyn Blocks, input: &Input) -> Outcome {
        let pts = &input.stroke;
        if pts.len() < 2 {
            return msg("Cetvel için en az iki nokta gerekli.");
        }
        let first = pts[0];
        let last = pts[pts.len() - 1];
        if self.select_center {
            // The block the middle of the line (first to last) lies in.
            let mid = [0, 1, 2].map(|i| (i64::from(first[i]) + i64::from(last[i])).div_euclid(2) as i32);
            let mut one = BTreeSet::new();
            one.insert(mid);
            return Outcome::Select(one);
        }
        let d = diff(first, last);
        let euclid = length(d);
        let manhattan: i64 = d.iter().map(|v| v.abs()).sum();
        let mut path = 0.0;
        for w in pts.windows(2) {
            path += length(diff(w[0], w[1]));
        }
        let lo = [0, 1, 2].map(|i| pts.iter().map(|p| p[i]).min().unwrap_or(0));
        let hi = [0, 1, 2].map(|i| pts.iter().map(|p| p[i]).max().unwrap_or(0));
        let size = [0, 1, 2].map(|i| i64::from(hi[i]) - i64::from(lo[i]) + 1);
        let mut lines = vec![
            format!("Cetvel: {} nokta", pts.len()),
            format!("Öklid uzaklığı (ilk-son): {euclid:.2} (yuvarlak {})", euclid.round() as i64),
            format!("Manhattan uzaklığı (ilk-son): {manhattan}"),
            format!("Fark: dx={} dy={} dz={}", d[0], d[1], d[2]),
            format!("Blok sayısı: x={} y={} z={}", d[0].abs() + 1, d[1].abs() + 1, d[2].abs() + 1),
        ];
        if pts.len() > 2 {
            lines.push(format!("Toplam yol uzunluğu: {path:.2}"));
        }
        lines.push(format!("En küçük: {}", pos_text(lo)));
        lines.push(format!("En büyük: {}", pos_text(hi)));
        lines.push(format!("Sınırlayıcı boyut: {} x {} x {}", size[0], size[1], size[2]));
        if pts.len() == 2 {
            let (dx, dy, dz) = (d[0] as f64, d[1] as f64, d[2] as f64);
            let yaw = (-dx).atan2(dz).to_degrees();
            let pitch = (-dy).atan2((dx * dx + dz * dz).sqrt()).to_degrees();
            lines.push(format!("Yaw: {yaw:.1}"));
            lines.push(format!("Pitch: {pitch:.1}"));
        }
        if self.circle {
            lines.push(format!("Daire: yarıçap {euclid:.2}, çevre {:.2}", euclid * std::f64::consts::TAU));
        }
        if self.split > 1 {
            let n = i64::from(self.split);
            'segments: for (si, w) in pts.windows(2).enumerate() {
                let sd = diff(w[0], w[1]);
                for k in 1..n {
                    if lines.len() >= RULER_LINES_MAX {
                        lines.push("… (daha fazla bölme noktası gösterilmiyor)".to_string());
                        break 'segments;
                    }
                    let at = [0, 1, 2].map(|i| {
                        let v = i64::from(w[0][i]) as f64 + sd[i] as f64 * k as f64 / n as f64;
                        v.round() as i32
                    });
                    lines.push(format!("Bölme {}/{} (çizgi {}): {}", k, n, si + 1, pos_text(at)));
                }
            }
        }
        Outcome::Message(lines)
    }
}

// ------------------------------------------------------------- Annotation Tool

/// The annotations placed so far. Rendering is elsewhere; this is where it
/// reads them from.
static ANNOTATIONS: Mutex<Vec<(Pos, String)>> = Mutex::new(Vec::new());

fn store() -> MutexGuard<'static, Vec<(Pos, String)>> {
    ANNOTATIONS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Every annotation: position and text.
pub fn annotations() -> Vec<(Pos, String)> {
    store().clone()
}

/// Adds a text annotation (cut to `TEXT_MAX` characters); false if the store
/// is full or the text is empty.
pub fn add_annotation(at: Pos, text: &str) -> bool {
    let text: String = text.trim().chars().take(TEXT_MAX).collect();
    let mut s = store();
    if text.is_empty() || s.len() >= ANNOTATION_MAX {
        return false;
    }
    s.push((at, text));
    true
}

/// Removes every annotation inside the box `lo..=hi`; how many went.
pub fn remove_annotations_in(lo: Pos, hi: Pos) -> usize {
    let mut s = store();
    let before = s.len();
    s.retain(|(p, _)| !(0..3).all(|i| p[i] >= lo[i] && p[i] <= hi[i]));
    before - s.len()
}

/// Removes all annotations ("Clear ALL"); how many went. It cannot be undone.
pub fn clear_annotations() -> usize {
    let mut s = store();
    let n = s.len();
    s.clear();
    n
}

/// Moves the annotation nearest to `from` (within `reach` blocks) to `to`;
/// the position it was at.
pub fn move_annotation(from: Pos, to: Pos, reach: i32) -> Option<Pos> {
    let mut s = store();
    let reach2 = i64::from(reach) * i64::from(reach);
    let mut best: Option<(usize, i64)> = None;
    for (i, (p, _)) in s.iter().enumerate() {
        let d = diff(from, *p);
        let d2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        if d2 <= reach2 && best.map_or(true, |(_, b)| d2 < b) {
            best = Some((i, d2));
        }
    }
    let (i, _) = best?;
    let old = s[i].0;
    s[i].0 = to;
    Some(old)
}

/// What the annotation tool does with a stroke.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnotationMode {
    /// Text: place `Input.text` at the first hit block.
    Text,
    /// Erase: remove the annotations inside the stroke's box.
    Erase,
    /// Move: take the annotation at the first hit to the last hit.
    Move,
    /// Clear ALL annotations.
    ClearAll,
}

impl AnnotationMode {
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "yaz" | "metin" | "text" => AnnotationMode::Text,
            "sil" | "erase" => AnnotationMode::Erase,
            "tasi" | "taşı" | "move" => AnnotationMode::Move,
            "hepsini-sil" | "hepsi" | "clear" => AnnotationMode::ClearAll,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            AnnotationMode::Text => "yaz",
            AnnotationMode::Erase => "sil",
            AnnotationMode::Move => "tasi",
            AnnotationMode::ClearAll => "hepsini-sil",
        }
    }
}

/// How a text faces the player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Billboard {
    Fixed,
    Vertical,
    Horizontal,
    Center,
}

impl Billboard {
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "sabit" | "fixed" => Billboard::Fixed,
            "dikey" | "vertical" => Billboard::Vertical,
            "yatay" | "horizontal" => Billboard::Horizontal,
            "merkez" | "center" => Billboard::Center,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Billboard::Fixed => "sabit",
            Billboard::Vertical => "dikey",
            Billboard::Horizontal => "yatay",
            Billboard::Center => "merkez",
        }
    }
}

/// Annotation tool: text annotations in the world (the text, erase and move
/// types of the docs; drawing, outlines and images need rendering and are not
/// here).
#[derive(Clone, Debug)]
pub struct Annotation {
    /// Inferred: the docs list no default; text.
    pub mode: AnnotationMode,
    /// Text shadow. Inferred default: off. Kept for the renderer; the store
    /// holds only position and text.
    pub shadow: bool,
    /// Inferred default: center. Kept for the renderer, like `shadow`.
    pub billboard: Billboard,
}

impl Default for Annotation {
    fn default() -> Self {
        Annotation { mode: AnnotationMode::Text, shadow: false, billboard: Billboard::Center }
    }
}

/// How far from the first hit a move looks for an annotation to take.
const MOVE_REACH: i32 = 3;

impl Tool for Annotation {
    fn id(&self) -> &'static str {
        "not"
    }
    fn name(&self) -> &'static str {
        "Not Aracı"
    }
    fn set(&mut self, option: &str, value: Option<&str>) -> Result<(), String> {
        match option {
            "mod" => {
                self.mode = value
                    .and_then(AnnotationMode::parse)
                    .ok_or_else(|| "yaz, sil, tasi ya da hepsini-sil bekleniyordu.".to_string())?
            }
            "golge" => toggle(&mut self.shadow, value)?,
            "billboard" => {
                self.billboard = value
                    .and_then(Billboard::parse)
                    .ok_or_else(|| "sabit, dikey, yatay ya da merkez bekleniyordu.".to_string())?
            }
            _ => return Err(unknown(option)),
        }
        Ok(())
    }
    fn describe(&self) -> Vec<String> {
        vec![
            format!("mod: {}", self.mode.name()),
            format!("golge: {}", on_off(self.shadow)),
            format!("billboard: {}", self.billboard.name()),
        ]
    }
    fn apply(&self, _world: &dyn Blocks, input: &Input) -> Outcome {
        let Some(&first) = input.stroke.first() else {
            return msg("Bir bloğa bakın.");
        };
        let last = input.stroke[input.stroke.len() - 1];
        // An annotation sits in front of the face that was looked at.
        let anchor = |p: Pos| add(p, input.face);
        match self.mode {
            AnnotationMode::Text => {
                let at = anchor(first);
                if input.text.trim().is_empty() {
                    return msg("Not metni boş olamaz.");
                }
                if add_annotation(at, &input.text) {
                    msg(format!("Not eklendi ({}): {}", pos_text(at), input.text.trim()))
                } else {
                    msg(format!("Not eklenemedi: en çok {ANNOTATION_MAX} not."))
                }
            }
            AnnotationMode::Erase => {
                // Inferred: the box drawn on screen is the box around the
                // stroke's hits, one block wider (notes sit beside blocks).
                let lo = [0, 1, 2].map(|i| input.stroke.iter().map(|p| p[i]).min().unwrap_or(0).saturating_sub(1));
                let hi = [0, 1, 2].map(|i| input.stroke.iter().map(|p| p[i]).max().unwrap_or(0).saturating_add(1));
                let n = remove_annotations_in(lo, hi);
                msg(format!("{n} not silindi."))
            }
            AnnotationMode::Move => {
                if input.stroke.len() < 2 {
                    return msg("Taşımak için ilk vuruş notu, son vuruş hedefi seçer.");
                }
                match move_annotation(anchor(first), anchor(last), MOVE_REACH) {
                    Some(old) => msg(format!("Not taşındı: {} -> {}", pos_text(old), pos_text(anchor(last)))),
                    None => msg("Yakında taşınacak not yok."),
                }
            }
            AnnotationMode::ClearAll => {
                let n = clear_annotations();
                msg(format!("Tüm notlar silindi ({n}); bu geri alınamaz."))
            }
        }
    }
}

/// The tools of this module, in the order the documentation lists them.
pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(BoxSelect::default()),
        Box::new(Freehand::default()),
        Box::new(Lasso::default()),
        Box::new(Ruler::default()),
        Box::new(Annotation::default()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct World(HashMap<Pos, Block>);

    fn b(id: &str, solid: bool) -> Block {
        Block { id: format!("minecraft:{id}"), state: String::new(), solid }
    }

    impl World {
        /// Air in a 41 block cube, with a stone cube from 0 to 6 on each axis.
        fn new() -> Self {
            let mut m = HashMap::new();
            for x in -20..=20 {
                for y in -20..=20 {
                    for z in -20..=20 {
                        let inside = (0..=6).contains(&x) && (0..=6).contains(&y) && (0..=6).contains(&z);
                        m.insert([x, y, z], if inside { b("stone", true) } else { b("air", false) });
                    }
                }
            }
            World(m)
        }
    }

    impl Blocks for World {
        fn block(&self, at: Pos) -> Option<Block> {
            self.0.get(&at).cloned()
        }
    }

    fn select(o: Outcome) -> BTreeSet<Pos> {
        match o {
            Outcome::Select(s) => s,
            other => panic!("expected a selection, got {other:?}"),
        }
    }

    fn lines(o: Outcome) -> Vec<String> {
        match o {
            Outcome::Message(l) => l,
            other => panic!("expected a message, got {other:?}"),
        }
    }

    fn input(stroke: &[Pos]) -> Input {
        Input { stroke: stroke.to_vec(), ..Input::default() }
    }

    #[test]
    fn list_and_ids() {
        let ids: Vec<&str> = tools().iter().map(|t| t.id()).collect();
        assert_eq!(ids, vec!["kutusecim", "elsecim", "kement", "cetvel", "not"]);
    }

    #[test]
    fn box_from_stroke_and_from_typed_corners() {
        let w = World::new();
        let mut t = BoxSelect::default();
        let got = select(t.apply(&w, &input(&[[0, 0, 0], [5, 5, 5], [2, 3, 1]])));
        assert_eq!(got.len(), 3 * 4 * 2);
        assert!(got.contains(&[0, 0, 0]) && got.contains(&[2, 3, 1]));
        t.set("pos1", Some("5,5,5")).unwrap();
        t.set("pos2", Some("3, 6, 5")).unwrap();
        let typed = select(t.apply(&w, &input(&[[0, 0, 0], [9, 9, 9]])));
        assert_eq!(typed.len(), 3 * 2);
        assert!(t.set("pos1", Some("1,2")).is_err());
        t.set("pos1", Some("yok")).unwrap();
        assert_eq!(t.describe()[0], "pos1: yok");
        assert!(matches!(t.apply(&w, &input(&[])), Outcome::Message(_)));
    }

    #[test]
    fn box_too_big_is_reported() {
        let w = World::new();
        let t = BoxSelect { pos1: Some([0, 0, 0]), pos2: Some([1000, 1000, 1000]) };
        let l = lines(t.apply(&w, &input(&[])));
        assert!(l[0].contains("çok büyük"));
    }

    #[test]
    fn freehand_paints_the_brush_along_the_stroke() {
        let w = World::new();
        let mut t = Freehand::default();
        t.set("yaricap", Some("1")).unwrap();
        t.set("sekil", Some("küp")).unwrap();
        let got = select(t.apply(&w, &input(&[[0, 0, 0], [1, 0, 0]])));
        assert_eq!(got.len(), 4 * 3 * 3);
        assert!(t.set("yaricap", Some("99")).is_err());
        assert!(t.set("sekil", Some("piramit")).is_err());
        t.set("sekil", Some("kure")).unwrap();
        assert_eq!(select(t.apply(&w, &input(&[[0, 0, 0]]))).len(), 19);
    }

    #[test]
    fn freehand_is_capped() {
        let w = World::new();
        let t = Freehand { radius: 20, shape: Shape::Cube };
        let far: Vec<Pos> = (0..16).map(|i| [i * 100, 0, 0]).collect();
        assert!(lines(t.apply(&w, &input(&far)))[0].contains("çok büyük"));
        let t = Freehand { radius: 32, shape: Shape::Cube };
        let many: Vec<Pos> = (0..1000).map(|i| [i, 0, 0]).collect();
        assert!(lines(t.apply(&w, &input(&many)))[0].contains("çok"));
    }

    fn lasso_input(look: [f64; 3]) -> Input {
        Input { stroke: vec![[0, 3, 0], [4, 3, 0], [4, 3, 4], [0, 3, 4]], look, ..Input::default() }
    }

    #[test]
    fn lasso_casts_the_outline_along_the_view() {
        let w = World::new();
        let mut t = Lasso::default();
        t.set("derinlik", Some("2")).unwrap();
        // Looking down: columns x,z in 0..=4, from y 3 down two more.
        let got = select(t.apply(&w, &lasso_input([0.0, -1.0, 0.0])));
        assert_eq!(got.len(), 5 * 5 * 3);
        assert!(got.iter().all(|p| (0..=4).contains(&p[0]) && (0..=4).contains(&p[2]) && (1..=3).contains(&p[1])));
        // Looking up it widens upwards instead: y 3..=5.
        let up = select(t.apply(&w, &lasso_input([0.0, 1.0, 0.0])));
        assert!(up.iter().all(|p| (3..=5).contains(&p[1])));
        assert_eq!(up.len(), 75);
    }

    #[test]
    fn lasso_non_solid_and_degenerate_outlines() {
        let mut m = World::new();
        m.0.insert([2, 2, 2], b("poppy", false));
        m.0.insert([1, 2, 2], b("air", false));
        let mut t = Lasso { depth: 2, non_solid: false };
        let look = [0.0, -1.0, 0.0];
        assert_eq!(select(t.apply(&m, &lasso_input(look))).len(), 75 - 2);
        t.set("katisiz", Some("ac")).unwrap();
        // The flower counts now, the air still does not.
        let got = select(t.apply(&m, &lasso_input(look)));
        assert_eq!(got.len(), 75 - 1);
        assert!(got.contains(&[2, 2, 2]) && !got.contains(&[1, 2, 2]));
        // A triangle selects a part of the square, and two points nothing.
        let tri = Input { stroke: vec![[0, 3, 0], [6, 3, 0], [0, 3, 6]], look, ..Input::default() };
        let part = select(t.apply(&m, &tri));
        assert!(part.contains(&[1, 3, 1]) && !part.contains(&[6, 3, 6]));
        let two = Input { stroke: vec![[0, 3, 0], [4, 3, 0]], look, ..Input::default() };
        assert!(matches!(t.apply(&m, &two), Outcome::Message(_)));
    }

    #[test]
    fn ruler_measures_two_points() {
        let w = World::new();
        let t = Ruler::default();
        let l = lines(t.apply(&w, &input(&[[0, 0, 0], [3, 4, 0]]))).join("\n");
        assert!(l.contains("5.00"), "{l}");
        assert!(l.contains("Manhattan uzaklığı (ilk-son): 7"), "{l}");
        assert!(l.contains("dx=3 dy=4 dz=0"), "{l}");
        assert!(l.contains("Blok sayısı: x=4 y=5 z=1"), "{l}");
        assert!(l.contains("Sınırlayıcı boyut: 4 x 5 x 1"), "{l}");
        assert!(l.contains("Yaw: -90.0"), "{l}");
        assert!(!l.contains("Toplam yol"), "{l}");
        let up = lines(t.apply(&w, &input(&[[0, 0, 0], [0, 5, 0]]))).join("\n");
        assert!(up.contains("Pitch: -90.0"), "{up}");
        assert!(matches!(t.apply(&w, &input(&[[0, 0, 0]])), Outcome::Message(_)));
    }

    #[test]
    fn ruler_path_circle_split_and_center() {
        let w = World::new();
        let mut t = Ruler::default();
        let pts: [Pos; 3] = [[0, 0, 0], [3, 4, 0], [3, 4, 12]];
        let l = lines(t.apply(&w, &input(&pts))).join("\n");
        assert!(l.contains("Toplam yol uzunluğu: 17.00"), "{l}");
        assert!(l.contains("13.00"), "{l}");
        assert!(!l.contains("Yaw"), "{l}");
        t.set("daire", None).unwrap();
        t.set("bol", Some("2")).unwrap();
        let l = lines(t.apply(&w, &input(&[[0, 0, 0], [10, 0, 0]]))).join("\n");
        assert!(l.contains("Daire: yarıçap 10.00"), "{l}");
        assert!(l.contains("Bölme 1/2 (çizgi 1): 5,0,0"), "{l}");
        t.set("merkezsec", Some("ac")).unwrap();
        let c = select(t.apply(&w, &input(&[[0, 0, 0], [10, 0, 0]])));
        assert_eq!(c, BTreeSet::from([[5, 0, 0]]));
        assert!(t.set("bol", Some("0")).is_err());
    }

    #[test]
    fn annotations_add_move_erase_clear() {
        // One test owns the global store, so no other test can disturb it.
        let w = World::new();
        clear_annotations();
        let mut t = Annotation::default();
        let mut inp = Input { stroke: vec![[100, 0, 0]], face: [0, 1, 0], text: "  giriş  ".into(), ..Input::default() };
        let l = lines(t.apply(&w, &inp));
        assert!(l[0].contains("Not eklendi") && l[0].contains("100,1,0"), "{l:?}");
        assert_eq!(annotations(), vec![([100, 1, 0], "giriş".to_string())]);
        inp.text = "   ".into();
        assert!(lines(t.apply(&w, &inp))[0].contains("boş"));
        assert_eq!(annotations().len(), 1);
        // Move: the first hit picks the note, the last is the target.
        t.set("mod", Some("tasi")).unwrap();
        let mv = Input { stroke: vec![[100, 0, 1], [110, 0, 0]], face: [0, 1, 0], ..Input::default() };
        assert!(lines(t.apply(&w, &mv))[0].contains("taşındı"));
        assert_eq!(annotations()[0].0, [110, 1, 0]);
        let far = Input { stroke: vec![[0, 0, 0], [5, 0, 0]], ..Input::default() };
        assert!(lines(t.apply(&w, &far))[0].contains("yok"));
        // Erase by the stroke's box.
        t.set("mod", Some("sil")).unwrap();
        add_annotation([300, 5, 5], "uzak");
        let er = Input { stroke: vec![[108, 0, 0], [112, 2, 2]], ..Input::default() };
        assert!(lines(t.apply(&w, &er))[0].starts_with("1 not"));
        assert_eq!(annotations(), vec![([300, 5, 5], "uzak".to_string())]);
        // Clear all.
        t.set("mod", Some("hepsini-sil")).unwrap();
        assert!(lines(t.apply(&w, &er))[0].contains("geri alınamaz"));
        assert!(annotations().is_empty());
        // Long text is cut.
        assert!(add_annotation([0, 0, 0], &"a".repeat(1000)));
        assert_eq!(annotations()[0].1.chars().count(), TEXT_MAX);
        assert!(!add_annotation([0, 0, 0], ""));
        assert_eq!(remove_annotations_in([0, 0, 0], [0, 0, 0]), 1);
        assert!(t.set("mod", Some("boya")).is_err());
        assert!(t.set("billboard", Some("dikey")).is_ok());
    }
}
