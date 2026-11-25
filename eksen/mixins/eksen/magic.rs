// Eksen — 16.10.1 (mihehk): Magic Select and the selection set it builds.
// Written from scratch after the behaviour Axiom's documentation describes
// (axiomdocs.moulberry.com, "Selection Tools" and "Magic Select"); no code,
// names or images of Axiom are used.
//
// Clicked on a block, it grows outwards from that block over blocks that
// match it, nearest first (so a limit cuts it as an octahedron), and joins
// the result to the selection by a boolean operation. Everything here is
// plain Rust over the `Blocks` trait: the game supplies the blocks.
use std::collections::{BTreeSet, HashSet, VecDeque};

pub type Pos = [i32; 3];

/// A block as Magic Select compares it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    /// `minecraft:oak_stairs`.
    pub id: String,
    /// Its properties as the game writes them, `facing=east,half=top`; empty
    /// for a block without any.
    pub state: String,
    /// Not passable: stone, glass panes; not flowers or water.
    pub solid: bool,
}

impl Block {
    pub fn air(&self) -> bool {
        matches!(&*self.id, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
    }
}

/// The world's blocks; None where they are not loaded.
pub trait Blocks {
    fn block(&self, at: Pos) -> Option<Block>;
    /// The game's id of a block string (`minecraft:oak_stairs[facing=east]`),
    /// for edits sent as ids (AxiomPaper). None: unknown, or not the game's.
    fn state_id(&self, _block: &str) -> Option<u32> {
        None
    }
    /// The dimension the player is in (`minecraft:overworld`).
    fn dimension(&self) -> Option<String> {
        None
    }
}

/// Which blocks count as the same as the one clicked ("Compare").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compare {
    /// The same block, in any of its states (the default).
    Block,
    /// The same block in exactly the same state.
    BlockState,
    /// Any block that cannot be walked through.
    Solid,
    /// Any block but air: it stops where air begins.
    Any,
}

impl Compare {
    fn matches(self, seed: &Block, other: &Block) -> bool {
        match self {
            Compare::Block => seed.id == other.id,
            Compare::BlockState => seed.id == other.id && seed.state == other.state,
            Compare::Solid => other.solid,
            Compare::Any => !other.air(),
        }
    }

    pub fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "blok" | "block" => Compare::Block,
            "durum" | "blockstate" | "state" => Compare::BlockState,
            "kati" | "katı" | "solid" => Compare::Solid,
            "herhangi" | "any" => Compare::Any,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Compare::Block => "blok",
            Compare::BlockState => "durum",
            Compare::Solid => "katı",
            Compare::Any => "herhangi",
        }
    }
}

/// Magic Select's options, with the documented defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub compare: Compare,
    /// The most blocks one click selects (default 100 000, any number).
    pub limit: usize,
    /// How far it looks for the next matching block, so it can jump air
    /// gaps (a tree's leaves across its branches). 1: touching blocks only.
    pub range: i32,
    /// Only the selected blocks that touch air.
    pub surface_only: bool,
    /// Only blocks at or above the clicked one.
    pub up: bool,
    /// Only blocks at or below the clicked one.
    pub down: bool,
    /// Grows sideways, not only up and down.
    pub horizontal: bool,
    /// With `horizontal`: also blocks that only meet at an edge or corner,
    /// within `range`.
    pub corners: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            compare: Compare::Block,
            limit: 100_000,
            range: 1,
            surface_only: false,
            up: false,
            down: false,
            horizontal: true,
            corners: false,
        }
    }
}

/// The largest range: past it a click looks at too many blocks per step.
pub const RANGE_MAX: i32 = 8;

/// The steps from one block to the next it may reach: straight along the
/// axes up to `range` blocks (sideways only with `horizontal`), and with
/// `corners` every block of the cube of that range.
fn steps(o: &Options) -> Vec<Pos> {
    let r = o.range.clamp(1, RANGE_MAX);
    let mut out = Vec::new();
    for dx in -r..=r {
        for dy in -r..=r {
            for dz in -r..=r {
                if (dx, dy, dz) == (0, 0, 0) {
                    continue;
                }
                let sideways = dx != 0 || dz != 0;
                if sideways && !o.horizontal {
                    continue;
                }
                let axes = [dx, dy, dz].iter().filter(|v| **v != 0).count();
                if axes == 1 || (o.corners && o.horizontal) {
                    out.push([dx, dy, dz]);
                }
            }
        }
    }
    // Nearest first, so the search front stays an octahedron.
    out.sort_by_key(|d| d[0].abs() + d[1].abs() + d[2].abs());
    out
}

fn add(a: Pos, d: Pos) -> Pos {
    [a[0] + d[0], a[1] + d[1], a[2] + d[2]]
}

/// One click (or one drag) from `seeds`: every block reachable from them over
/// blocks that match one of the seeds, nearest first, at most `o.limit`.
pub fn select(world: &dyn Blocks, seeds: &[Pos], o: &Options) -> BTreeSet<Pos> {
    let kinds: Vec<Block> = seeds.iter().filter_map(|p| world.block(*p)).filter(|b| !b.air()).collect();
    let Some(&origin) = seeds.first() else {
        return BTreeSet::new();
    };
    if kinds.is_empty() || o.limit == 0 {
        return BTreeSet::new();
    }
    let within = |p: Pos| (!o.up || p[1] >= origin[1]) && (!o.down || p[1] <= origin[1]);
    let matches = |b: &Block| kinds.iter().any(|k| o.compare.matches(k, b));
    let steps = steps(o);
    let mut seen: HashSet<Pos> = HashSet::new();
    let mut found = BTreeSet::new();
    let mut front = VecDeque::new();
    for s in seeds {
        if seen.insert(*s) && within(*s) && world.block(*s).is_some_and(|b| matches(&b)) {
            front.push_back(*s);
        }
    }
    while let Some(p) = front.pop_front() {
        found.insert(p);
        if found.len() >= o.limit {
            break;
        }
        for d in &steps {
            let q = add(p, *d);
            if !within(q) || !seen.insert(q) {
                continue;
            }
            if world.block(q).is_some_and(|b| matches(&b)) {
                front.push_back(q);
            }
        }
    }
    if o.surface_only {
        let faces = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
        found.retain(|p| faces.iter().any(|d| world.block(add(*p, *d)).is_none_or(|b| b.air())));
    }
    found
}

/// How a new selection joins the one there is ("boolean operation";
/// default: add).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Subtract,
    Replace,
    Intersect,
}

impl Op {
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "ekle" | "add" => Op::Add,
            "cikar" | "çıkar" | "subtract" => Op::Subtract,
            "yerine" | "replace" => Op::Replace,
            "kesisim" | "kesişim" | "intersect" => Op::Intersect,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Op::Add => "ekle",
            Op::Subtract => "çıkar",
            Op::Replace => "yerine",
            Op::Intersect => "kesişim",
        }
    }

    pub fn apply(self, current: &mut BTreeSet<Pos>, new: BTreeSet<Pos>) {
        match self {
            Op::Add => current.extend(new),
            Op::Subtract => current.retain(|p| !new.contains(p)),
            Op::Replace => *current = new,
            Op::Intersect => current.retain(|p| new.contains(p)),
        }
    }
}

/// The set as rows along x: `(start, end)` of each unbroken run, so a whole
/// row goes in one `fill`.
pub fn runs(set: &BTreeSet<Pos>) -> Vec<(Pos, Pos)> {
    // Ordered by (y, z, x) so a row's blocks come one after another.
    let mut rows: Vec<Pos> = set.iter().copied().collect();
    rows.sort_by_key(|p| (p[1], p[2], p[0]));
    let mut out: Vec<(Pos, Pos)> = Vec::new();
    for p in rows {
        match out.last_mut() {
            Some((_, end)) if end[1] == p[1] && end[2] == p[2] && end[0] + 1 == p[0] => *end = p,
            _ => out.push((p, p)),
        }
    }
    out
}

/// The smallest box around the set.
pub fn bounds(set: &BTreeSet<Pos>) -> Option<(Pos, Pos)> {
    let mut it = set.iter();
    let first = *it.next()?;
    Some(it.fold((first, first), |(lo, hi), p| {
        ([0, 1, 2].map(|i| lo[i].min(p[i])), [0, 1, 2].map(|i| hi[i].max(p[i])))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct World(HashMap<Pos, Block>);

    fn b(id: &str, state: &str, solid: bool) -> Block {
        Block { id: format!("minecraft:{id}"), state: state.into(), solid }
    }

    impl World {
        /// Air everywhere in a 41 block cube, with `set` placed.
        fn new(set: &[(Pos, Block)]) -> Self {
            let mut m = HashMap::new();
            for x in -20..=20 {
                for y in -20..=20 {
                    for z in -20..=20 {
                        m.insert([x, y, z], b("air", "", false));
                    }
                }
            }
            for (p, block) in set {
                m.insert(*p, block.clone());
            }
            World(m)
        }
    }

    impl Blocks for World {
        fn block(&self, at: Pos) -> Option<Block> {
            self.0.get(&at).cloned()
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

    #[test]
    fn it_takes_the_touching_blocks_of_the_same_kind() {
        let stone = b("stone", "", true);
        let mut set = cube(2, &stone);
        set.push(([3, 0, 0], b("dirt", "", true)));
        set.push(([5, 0, 0], stone.clone()));
        let w = World::new(&set);
        let got = select(&w, &[[0, 0, 0]], &Options::default());
        assert_eq!(got.len(), 125);
        assert!(!got.contains(&[5, 0, 0]), "past a gap at range 1");
        // With range 3 the far stone is reached over the dirt.
        let got = select(&w, &[[0, 0, 0]], &Options { range: 3, ..Options::default() });
        assert!(got.contains(&[5, 0, 0]));
    }

    #[test]
    fn compare_modes() {
        let up = b("oak_stairs", "half=top", true);
        let down = b("oak_stairs", "half=bottom", true);
        let glass = b("glass", "", true);
        let flower = b("poppy", "", false);
        let w = World::new(&[([0, 0, 0], up.clone()), ([1, 0, 0], down), ([2, 0, 0], glass), ([3, 0, 0], flower)]);
        let n = |compare| select(&w, &[[0, 0, 0]], &Options { compare, ..Options::default() }).len();
        assert_eq!(n(Compare::BlockState), 1);
        assert_eq!(n(Compare::Block), 2);
        assert_eq!(n(Compare::Solid), 3);
        assert_eq!(n(Compare::Any), 4);
    }

    #[test]
    fn the_limit_cuts_an_octahedron_around_the_click() {
        let w = World::new(&cube(10, &b("stone", "", true)));
        let got = select(&w, &[[0, 0, 0]], &Options { limit: 25, ..Options::default() });
        assert_eq!(got.len(), 25);
        // The 25 nearest by steps: everything within 2, nothing past 3.
        assert!(got.iter().all(|p| p[0].abs() + p[1].abs() + p[2].abs() <= 3));
        assert!(got.iter().filter(|p| p[0].abs() + p[1].abs() + p[2].abs() <= 2).count() == 25);
    }

    #[test]
    fn up_down_horizontal_corners_and_surface() {
        let stone = b("stone", "", true);
        let w = World::new(&cube(2, &stone));
        let o = Options::default();
        assert!(select(&w, &[[0, 0, 0]], &Options { up: true, ..o.clone() }).iter().all(|p| p[1] >= 0));
        assert!(select(&w, &[[0, 0, 0]], &Options { down: true, ..o.clone() }).iter().all(|p| p[1] <= 0));
        // Without horizontal: only the column.
        let column = select(&w, &[[0, 0, 0]], &Options { horizontal: false, ..o.clone() });
        assert_eq!(column.len(), 5);
        // Corners: two blocks that only meet at a corner.
        let w2 = World::new(&[([0, 0, 0], stone.clone()), ([1, 1, 1], stone.clone())]);
        assert_eq!(select(&w2, &[[0, 0, 0]], &o).len(), 1);
        assert_eq!(select(&w2, &[[0, 0, 0]], &Options { corners: true, ..o.clone() }).len(), 2);
        // Surface: a 5 cube's shell is 125 - 27.
        assert_eq!(select(&w, &[[0, 0, 0]], &Options { surface_only: true, ..o }).len(), 98);
    }

    #[test]
    fn a_drag_over_two_kinds_takes_both() {
        let w = World::new(&[([0, 0, 0], b("stone", "", true)), ([1, 0, 0], b("dirt", "", true)), ([2, 0, 0], b("sand", "", true))]);
        assert_eq!(select(&w, &[[0, 0, 0], [1, 0, 0]], &Options::default()).len(), 2);
    }

    #[test]
    fn boolean_operations_and_rows() {
        let a: BTreeSet<Pos> = [[0, 0, 0], [1, 0, 0], [2, 0, 0]].into();
        let b: BTreeSet<Pos> = [[2, 0, 0], [3, 0, 0]].into();
        let run = |op: Op| {
            let mut s = a.clone();
            op.apply(&mut s, b.clone());
            s.len()
        };
        assert_eq!((run(Op::Add), run(Op::Subtract), run(Op::Replace), run(Op::Intersect)), (4, 2, 2, 1));
        let mut s = a.clone();
        s.insert([5, 0, 0]);
        s.insert([0, 1, 0]);
        assert_eq!(runs(&s), vec![([0, 0, 0], [2, 0, 0]), ([5, 0, 0], [5, 0, 0]), ([0, 1, 0], [0, 1, 0])]);
        assert_eq!(bounds(&s), Some(([0, 0, 0], [5, 1, 0])));
    }
}
