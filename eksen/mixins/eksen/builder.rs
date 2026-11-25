// Eksen — 16.10.1 (mihehk): the builder tools of the tenth hotbar slot
// (Move, Clone, Stack, Smear, Extrude, Erase, Setup Symmetry) and the
// undo / redo history. Written from scratch after the behaviour Axiom's
// documentation describes (.build-tools/axiom-docs/axiom-docs.md: "Builder
// Mode", "Builder Tools" … "Setup Symmetry"); no code, names or images of
// Axiom are used.
//
// Every function here is plain Rust over `Blocks` and returns the blocks to
// set; the editor turns them into commands and records them in `History`.
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

use super::eksen_core::{block_string, Blocks, Pos, AIR};

/// The flags most builder tools share.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flags {
    /// Never overwrite a block that is already there (only air is filled).
    pub keep_existing: bool,
    /// Air in the source is copied too (clearing what is at the target).
    pub copy_air: bool,
}

impl Default for Flags {
    // Inferred: the documentation names the flags but not their defaults.
    fn default() -> Self {
        Flags { keep_existing: false, copy_air: false }
    }
}

/// How a moved or cloned selection is turned: `rotate` quarter turns
/// clockwise seen from above (north → east), then flipped along the axes set
/// in `flip` (x, y, z).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Transform {
    pub rotate: u8,
    pub flip: [bool; 3],
}

/// A cuboid by its min and max corners (both inside).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cuboid {
    pub min: Pos,
    pub max: Pos,
}

impl Cuboid {
    pub fn new(a: Pos, b: Pos) -> Self {
        Cuboid { min: [0, 1, 2].map(|i| a[i].min(b[i])), max: [0, 1, 2].map(|i| a[i].max(b[i])) }
    }
    pub fn size(&self) -> Pos {
        [0, 1, 2].map(|i| self.max[i] - self.min[i] + 1)
    }
    pub fn volume(&self) -> i64 {
        self.size().iter().map(|v| i64::from(*v)).product()
    }
    pub fn blocks(&self) -> impl Iterator<Item = Pos> + '_ {
        (self.min[0]..=self.max[0]).flat_map(move |x| {
            (self.min[1]..=self.max[1]).flat_map(move |y| (self.min[2]..=self.max[2]).map(move |z| [x, y, z]))
        })
    }
    pub fn contains(&self, p: Pos) -> bool {
        (0..3).all(|i| self.min[i] <= p[i] && p[i] <= self.max[i])
    }
    /// Middle-click: grows the cuboid by one block out of `face` (a unit step).
    pub fn expand_face(&self, face: Pos) -> Cuboid {
        let mut c = *self;
        for i in 0..3 {
            if face[i] > 0 {
                c.max[i] += face[i];
            } else if face[i] < 0 {
                c.min[i] += face[i];
            }
        }
        c
    }
}

/// The most blocks one builder action touches.
pub const LIMIT: i64 = 4_000_000;

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// Where `p` (inside `src`) lands when `src` is turned by `t` and its min
/// corner is put at `to`.
pub fn place(src: &Cuboid, t: Transform, to: Pos, p: Pos) -> Pos {
    let s = src.size();
    let mut d = [p[0] - src.min[0], p[1] - src.min[1], p[2] - src.min[2]];
    let mut size = s;
    for _ in 0..t.rotate % 4 {
        // Clockwise from above: x' = (depth - 1) - z, z' = x.
        d = [size[2] - 1 - d[2], d[1], d[0]];
        size = [size[2], size[1], size[0]];
    }
    for i in 0..3 {
        if t.flip[i] {
            d[i] = size[i] - 1 - d[i];
        }
    }
    add(to, d)
}

/// The size of `src` once turned by `t`.
pub fn turned_size(src: &Cuboid, t: Transform) -> Pos {
    let s = src.size();
    if t.rotate % 2 == 1 {
        [s[2], s[1], s[0]]
    } else {
        s
    }
}

const FACINGS: [&str; 4] = ["north", "east", "south", "west"];

/// A block string turned with its blocks: `facing`, `axis`, `rotation`,
/// the fence sides and stair `shape` follow the rotation and flips.
pub fn turn_block(block: &str, t: Transform) -> String {
    let Some((id, props)) = block.strip_suffix(']').and_then(|b| b.split_once('[')) else {
        return block.to_owned();
    };
    let mut pairs: Vec<(String, String)> =
        props.split(',').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.to_owned(), v.to_owned())).collect();
    let turn_dir = |v: &str, quarter: u8| -> String {
        match FACINGS.iter().position(|f| *f == v) {
            Some(i) => FACINGS[(i + quarter as usize) % 4].to_owned(),
            None => v.to_owned(),
        }
    };
    let flip_dir = |v: &str| -> String {
        let flipped = match v {
            "east" if t.flip[0] => "west",
            "west" if t.flip[0] => "east",
            "up" if t.flip[1] => "down",
            "down" if t.flip[1] => "up",
            "south" if t.flip[2] => "north",
            "north" if t.flip[2] => "south",
            other => other,
        };
        flipped.to_owned()
    };
    let q = t.rotate % 4;
    let mirrored = (t.flip[0] as u8 + t.flip[2] as u8) % 2 == 1;
    let mut sides: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in &mut pairs {
        match k.as_str() {
            "facing" => *v = flip_dir(&turn_dir(v, q)),
            "axis" if q % 2 == 1 => {
                *v = match v.as_str() {
                    "x" => "z".into(),
                    "z" => "x".into(),
                    other => other.into(),
                }
            }
            "rotation" => {
                if let Ok(r) = v.parse::<i32>() {
                    let mut r = (r + 4 * i32::from(q)).rem_euclid(16);
                    if t.flip[0] {
                        r = (16 - r).rem_euclid(16);
                    }
                    if t.flip[2] {
                        r = (8 - r).rem_euclid(16);
                    }
                    *v = r.to_string();
                }
            }
            "half" if t.flip[1] => {
                *v = match v.as_str() {
                    "top" => "bottom".into(),
                    "bottom" => "top".into(),
                    other => other.into(),
                }
            }
            "type" if t.flip[1] => {
                *v = match v.as_str() {
                    "top" => "bottom".into(),
                    "bottom" => "top".into(),
                    other => other.into(),
                }
            }
            "shape" if mirrored => {
                *v = match v.as_str() {
                    "inner_left" => "inner_right".into(),
                    "inner_right" => "inner_left".into(),
                    "outer_left" => "outer_right".into(),
                    "outer_right" => "outer_left".into(),
                    other => other.into(),
                }
            }
            side @ ("north" | "east" | "south" | "west") => {
                sides.insert(flip_dir(&turn_dir(side, q)), v.clone());
            }
            _ => {}
        }
    }
    for (k, v) in &mut pairs {
        if let Some(n) = sides.get(k.as_str()) {
            *v = n.clone();
        }
    }
    let props: Vec<String> = pairs.into_iter().map(|(k, v)| format!("{k}={v}")).collect();
    format!("{id}[{}]", props.join(","))
}

/// The blocks of `src` (air too when `flags.copy_air`), turned by `t`, with
/// the min corner at `to`; existing blocks kept when `flags.keep_existing`.
pub fn copy(world: &dyn Blocks, src: &Cuboid, t: Transform, to: Pos, flags: Flags) -> Result<BTreeMap<Pos, String>, String> {
    if src.volume() > LIMIT {
        return Err(format!("Seçim çok büyük ({} blok, sınır {LIMIT}).", src.volume()));
    }
    let mut out = BTreeMap::new();
    for p in src.blocks() {
        let Some(b) = world.block(p) else { continue };
        if b.air() && !flags.copy_air {
            continue;
        }
        let q = place(src, t, to, p);
        if flags.keep_existing && world.block(q).is_some_and(|e| !e.air()) {
            continue;
        }
        out.insert(q, turn_block(&block_string(&b), t));
    }
    Ok(out)
}

/// Move: the blocks of `src` go to `to` (turned by `t`); what they leave
/// behind becomes air.
pub fn move_to(world: &dyn Blocks, src: &Cuboid, t: Transform, to: Pos, flags: Flags) -> Result<BTreeMap<Pos, String>, String> {
    let placed = copy(world, src, t, to, Flags { copy_air: true, ..flags })?;
    let mut out: BTreeMap<Pos, String> = src
        .blocks()
        .filter(|p| world.block(*p).is_some_and(|b| !b.air()))
        .map(|p| (p, AIR.to_owned()))
        .collect();
    for (p, b) in placed {
        // Air of the source only clears the target when copy air is on.
        if b == AIR && !flags.copy_air && !out.contains_key(&p) {
            continue;
        }
        out.insert(p, b);
    }
    Ok(out)
}

/// Clone, as many copies as `targets` (each a min corner).
pub fn clone_to(world: &dyn Blocks, src: &Cuboid, t: Transform, targets: &[Pos], flags: Flags) -> Result<BTreeMap<Pos, String>, String> {
    let mut out = BTreeMap::new();
    for to in targets {
        out.extend(copy(world, src, t, *to, flags)?);
    }
    Ok(out)
}

/// Stack: the min corners of `count` copies along `dir` (a unit step), each
/// one selection-size further; `count` negative stacks the other way.
pub fn stack_targets(src: &Cuboid, dir: Pos, count: i32) -> Vec<Pos> {
    let s = src.size();
    let sign = count.signum();
    (1..=count.abs())
        .map(|k| [0, 1, 2].map(|i| src.min[i] + dir[i] * s[i] * k * sign))
        .collect()
}

/// Stack to a point (middle-click): as many copies along the axis `dir` as
/// reach `point`.
pub fn stack_to_point(src: &Cuboid, dir: Pos, point: Pos) -> Vec<Pos> {
    let s = src.size();
    let axis = (0..3).find(|i| dir[*i] != 0).unwrap_or(0);
    let step = s[axis];
    let reach = if dir[axis] > 0 { point[axis] - src.max[axis] } else { src.min[axis] - point[axis] };
    let count = (reach + step - 1).div_euclid(step).max(0);
    stack_targets(src, dir, count)
}

/// Smear: the selection stretched `steps` blocks along `dir`: each column of
/// the selection along `dir` is repeated block by block, and a solid block
/// in its path cuts that column off.
pub fn smear(world: &dyn Blocks, src: &Cuboid, dir: Pos, steps: i32, flags: Flags) -> Result<BTreeMap<Pos, String>, String> {
    if src.volume() * i64::from(steps.abs().max(1)) > LIMIT {
        return Err("Sürme çok büyük.".into());
    }
    let mut out = BTreeMap::new();
    let mut cut: HashSet<Pos> = HashSet::new();
    for k in 1..=steps.max(0) {
        for p in src.blocks() {
            // The column a block belongs to: its position with the smear axis
            // removed.
            let col = [0, 1, 2].map(|i| if dir[i] != 0 { 0 } else { p[i] });
            if cut.contains(&col) {
                continue;
            }
            let Some(b) = world.block(p) else { continue };
            if b.air() && !flags.copy_air {
                continue;
            }
            let q = [0, 1, 2].map(|i| p[i] + dir[i] * k);
            if src.contains(q) {
                continue;
            }
            match world.block(q) {
                Some(e) if e.solid && !out.contains_key(&q) => {
                    cut.insert(col);
                }
                Some(e) => {
                    if !(flags.keep_existing && !e.air()) {
                        out.insert(q, block_string(&b));
                    }
                }
                None => {}
            }
        }
    }
    Ok(out)
}

/// The face of `start` looked at (`face`, a unit step out) and every block
/// of the same kind joined to it on that plane whose front is open.
fn face_blocks(world: &dyn Blocks, start: Pos, face: Pos, limit: usize) -> Vec<Pos> {
    let Some(kind) = world.block(start) else { return Vec::new() };
    let axis = (0..3).find(|i| face[*i] != 0).unwrap_or(1);
    let open = |p: Pos| world.block(add(p, face)).is_some_and(|b| !b.solid);
    let mut seen = HashSet::from([start]);
    let mut queue = VecDeque::from([start]);
    let mut out = Vec::new();
    while let Some(p) = queue.pop_front() {
        out.push(p);
        if out.len() >= limit {
            break;
        }
        for i in (0..3).filter(|i| *i != axis) {
            for s in [-1, 1] {
                let mut q = p;
                q[i] += s;
                if seen.insert(q) && world.block(q).is_some_and(|b| b.id == kind.id) && open(q) {
                    queue.push_back(q);
                }
            }
        }
    }
    out
}

/// The most blocks one extrude moves.
pub const EXTRUDE_MAX: usize = 4096;

/// Extrude: the looked-at face of the same block grows one layer out of the
/// face (right-click), or the layer behind it is removed (left-click,
/// `shrink`).
pub fn extrude(world: &dyn Blocks, start: Pos, face: Pos, shrink: bool) -> BTreeMap<Pos, String> {
    let blocks = face_blocks(world, start, face, EXTRUDE_MAX);
    let mut out = BTreeMap::new();
    for p in blocks {
        if shrink {
            out.insert(p, AIR.to_owned());
        } else if let Some(b) = world.block(p) {
            out.insert(add(p, face), block_string(&b));
        }
    }
    out
}

/// Erase connected (right-click): the nearest 128 blocks of the same kind.
pub const ERASE_CONNECTED: usize = 128;

pub fn erase_connected(world: &dyn Blocks, start: Pos) -> BTreeMap<Pos, String> {
    let options = super::eksen_magic::Options { limit: ERASE_CONNECTED, ..Default::default() };
    super::eksen_magic::select(world, &[start], &options).into_iter().map(|p| (p, AIR.to_owned())).collect()
}

/// Erase a cuboid.
pub fn erase(world: &dyn Blocks, src: &Cuboid) -> BTreeMap<Pos, String> {
    src.blocks().filter(|p| world.block(*p).is_some_and(|b| !b.air())).map(|p| (p, AIR.to_owned())).collect()
}

/// The symmetry node: at a block's centre, face, edge or corner (positions
/// in half blocks, so `2 * x`), with its flip and rotation modifiers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Symmetry {
    /// Twice the node's position: odd values sit between blocks.
    pub node2: Option<Pos>,
    pub flip: [bool; 3],
    /// Copies around the vertical axis through the node: 0 (off), 2 or 4.
    pub rotate: u8,
}

/// Symmetry nodes are dropped past this distance from the player.
pub const SYMMETRY_RANGE: f64 = 256.0;

impl Symmetry {
    /// On: a node and at least one modifier.
    pub fn on(&self) -> bool {
        self.node2.is_some() && (self.flip.iter().any(|f| *f) || self.rotate > 1)
    }

    /// Ctrl+R: none → 2 → 4 → none.
    pub fn cycle_rotate(&mut self) {
        self.rotate = match self.rotate {
            0 | 1 => 2,
            2 => 4,
            _ => 0,
        };
    }

    /// Every copy of block `p` (itself first).
    pub fn copies(&self, p: Pos) -> Vec<Pos> {
        let Some(n) = self.node2.filter(|_| self.on()) else { return vec![p] };
        // In half blocks around the node: a block's centre is 2p + 1.
        let c = [0, 1, 2].map(|i| 2 * p[i] + 1 - n[i]);
        let mut rel = vec![c];
        if self.rotate > 1 {
            let mut more = Vec::new();
            for r in &rel {
                let mut v = *r;
                for _ in 1..self.rotate {
                    v = if self.rotate == 4 { [-v[2], v[1], v[0]] } else { [-v[0], v[1], -v[2]] };
                    more.push(v);
                }
            }
            rel.extend(more);
        }
        for i in 0..3 {
            if self.flip[i] {
                let flipped: Vec<Pos> = rel
                    .iter()
                    .map(|v| {
                        let mut f = *v;
                        f[i] = -f[i];
                        f
                    })
                    .collect();
                rel.extend(flipped);
            }
        }
        let mut seen = BTreeSet::new();
        rel.into_iter()
            .map(|v| [0, 1, 2].map(|i| (v[i] + n[i] - 1).div_euclid(2)))
            .filter(|q| seen.insert(*q))
            .collect()
    }

    /// `edit` with every block mirrored by the node.
    pub fn apply(&self, edit: BTreeMap<Pos, String>) -> BTreeMap<Pos, String> {
        if !self.on() {
            return edit;
        }
        let mut out = BTreeMap::new();
        for (p, b) in edit {
            for q in self.copies(p) {
                out.entry(q).or_insert_with(|| b.clone());
            }
        }
        out
    }
}

/// One change: what each block was before and is after.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub before: BTreeMap<Pos, String>,
    pub after: BTreeMap<Pos, String>,
}

/// Ctrl+Z / Ctrl+Y.
#[derive(Clone, Debug, Default)]
pub struct History {
    done: Vec<Change>,
    undone: Vec<Change>,
}

/// Changes kept.
pub const HISTORY_MAX: usize = 64;

impl History {
    /// Records `after` as done, reading what was there from `world`.
    pub fn record(&mut self, world: &dyn Blocks, after: &BTreeMap<Pos, String>) {
        let before = after
            .keys()
            .filter_map(|p| world.block(*p).map(|b| (*p, block_string(&b))))
            .collect();
        self.done.push(Change { before, after: after.clone() });
        if self.done.len() > HISTORY_MAX {
            self.done.remove(0);
        }
        self.undone.clear();
    }

    /// The blocks that undo the last change.
    pub fn undo(&mut self) -> Option<BTreeMap<Pos, String>> {
        let c = self.done.pop()?;
        let edit = c.before.clone();
        self.undone.push(c);
        Some(edit)
    }

    /// The blocks that redo the last undone change.
    pub fn redo(&mut self) -> Option<BTreeMap<Pos, String>> {
        let c = self.undone.pop()?;
        let edit = c.after.clone();
        self.done.push(c);
        Some(edit)
    }

    pub fn len(&self) -> (usize, usize) {
        (self.done.len(), self.undone.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::eksen_magic::Block;
    use std::collections::HashMap;

    struct World(HashMap<Pos, Block>);

    impl Blocks for World {
        fn block(&self, at: Pos) -> Option<Block> {
            Some(self.0.get(&at).cloned().unwrap_or(Block { id: AIR.into(), state: String::new(), solid: false }))
        }
    }

    fn stone() -> Block {
        Block { id: "minecraft:stone".into(), state: String::new(), solid: true }
    }

    #[test]
    fn rotating_and_flipping_blocks_and_states() {
        let src = Cuboid::new([0, 0, 0], [2, 0, 0]);
        let t = Transform { rotate: 1, flip: [false; 3] };
        // A row along x turns into a row along z.
        assert_eq!(place(&src, t, [0, 0, 0], [2, 0, 0]), [0, 0, 2]);
        assert_eq!(turned_size(&src, t), [1, 1, 3]);
        assert_eq!(turn_block("minecraft:oak_stairs[facing=north,half=bottom]", t), "minecraft:oak_stairs[facing=east,half=bottom]");
        assert_eq!(turn_block("minecraft:oak_log[axis=x]", t), "minecraft:oak_log[axis=z]");
        let flip = Transform { rotate: 0, flip: [true, false, false] };
        assert_eq!(place(&src, flip, [0, 0, 0], [0, 0, 0]), [2, 0, 0]);
        assert_eq!(turn_block("minecraft:oak_stairs[facing=east,shape=inner_left]", flip), "minecraft:oak_stairs[facing=west,shape=inner_right]");
        assert_eq!(turn_block("minecraft:oak_fence[east=true,west=false]", flip), "minecraft:oak_fence[east=false,west=true]");
        assert_eq!(turn_block("minecraft:stone", t), "minecraft:stone");
    }

    #[test]
    fn move_clone_stack() {
        let w = World(HashMap::from([([0, 0, 0], stone()), ([1, 0, 0], stone())]));
        let src = Cuboid::new([0, 0, 0], [1, 0, 0]);
        let moved = move_to(&w, &src, Transform::default(), [5, 0, 0], Flags::default()).unwrap();
        assert_eq!(moved.get(&[0, 0, 0]).map(String::as_str), Some(AIR));
        assert_eq!(moved.get(&[5, 0, 0]).map(String::as_str), Some("minecraft:stone"));
        let cloned = clone_to(&w, &src, Transform::default(), &stack_targets(&src, [1, 0, 0], 3), Flags::default()).unwrap();
        assert_eq!(cloned.len(), 6);
        assert!(cloned.contains_key(&[7, 0, 0]) && !cloned.contains_key(&[0, 0, 0]));
        assert_eq!(stack_to_point(&src, [1, 0, 0], [6, 0, 0]).len(), 3);
    }

    #[test]
    fn smear_stops_at_solid_blocks() {
        let mut m = HashMap::from([([0, 0, 0], stone()), ([0, 1, 0], stone())]);
        m.insert([3, 1, 0], Block { id: "minecraft:dirt".into(), state: String::new(), solid: true });
        let w = World(m);
        let src = Cuboid::new([0, 0, 0], [0, 1, 0]);
        let out = smear(&w, &src, [1, 0, 0], 5, Flags::default()).unwrap();
        assert!(out.contains_key(&[5, 0, 0]));
        assert!(out.contains_key(&[2, 1, 0]) && !out.contains_key(&[3, 1, 0]) && !out.contains_key(&[4, 1, 0]));
    }

    #[test]
    fn extrude_and_erase() {
        let mut m = HashMap::new();
        for x in 0..3 {
            for z in 0..3 {
                m.insert([x, 0, z], stone());
            }
        }
        let w = World(m);
        let up = extrude(&w, [1, 0, 1], [0, 1, 0], false);
        assert_eq!(up.len(), 9);
        assert!(up.keys().all(|p| p[1] == 1));
        assert_eq!(extrude(&w, [1, 0, 1], [0, 1, 0], true).len(), 9);
        assert_eq!(erase_connected(&w, [0, 0, 0]).len(), 9);
        assert_eq!(erase(&w, &Cuboid::new([0, 0, 0], [1, 0, 0])).len(), 2);
    }

    #[test]
    fn symmetry_copies() {
        // Node on the face between x = 0 and x = 1 (half blocks: 2).
        let mut s = Symmetry { node2: Some([2, 0, 0]), flip: [true, false, false], rotate: 0 };
        assert_eq!(s.copies([0, 0, 0]), vec![[0, 0, 0], [1, 0, 0]]);
        assert_eq!(s.copies([3, 5, 2]), vec![[3, 5, 2], [-2, 5, 2]]);
        // Node at a block's centre, four-fold: a block 2 east gets 4 copies.
        s = Symmetry { node2: Some([1, 1, 1]), flip: [false; 3], rotate: 4 };
        let c = s.copies([2, 0, 0]);
        assert_eq!(c.len(), 4);
        assert!(c.contains(&[0, 0, 2]) && c.contains(&[-2, 0, 0]) && c.contains(&[0, 0, -2]));
        s.cycle_rotate();
        assert!(!s.on());
    }

    #[test]
    fn undo_redo() {
        let w = World(HashMap::from([([0, 0, 0], stone())]));
        let mut h = History::default();
        let edit = BTreeMap::from([([0, 0, 0], AIR.to_owned())]);
        h.record(&w, &edit);
        assert_eq!(h.undo().unwrap().get(&[0, 0, 0]).map(String::as_str), Some("minecraft:stone"));
        assert_eq!(h.redo().unwrap().get(&[0, 0, 0]).map(String::as_str), Some(AIR));
        assert_eq!(h.len(), (1, 0));
    }
}
