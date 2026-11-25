// Eksen — 16.10.2 (mihehk): a building editor for Horizon. This module holds the
// selection, the clipboard, the brush and the command queue, and turns editor
// actions into vanilla commands (`fill`, `clone`) the server runs. The hooks
// in this bundle only read and write this state and call the game's helpers.
// The selection is a box (two corners) or a set of blocks (Magic Select,
// `eksen_magic`).
use std::collections::{BTreeSet, VecDeque};
use std::sync::{Mutex, MutexGuard};

use super::eksen_magic as magic;

pub const ID: &str = "eksen";
pub const NAME: &str = "Eksen";
pub const VERSION: &str = "16.10.2";
pub const DESCRIPTION: &str = "Yapı editörü: uzaktan seçim, Magic Select, doldur, değiştir, kopyala/yapıştır, çoğalt, fırça. /eksen yardim";

/// Mod keys: name, label, default key.
pub const KEYS: [(&str, &str, &str); 9] = [
    ("pos1", "Eksen: 1. köşe (bakılan blok)", "key.keyboard.j"),
    ("pos2", "Eksen: 2. köşe (bakılan blok)", "key.keyboard.k"),
    ("clear", "Eksen: seçimi temizle", "key.keyboard.u"),
    ("brush", "Eksen: fırça (basılı tut)", "key.keyboard.h"),
    ("magic", "Eksen: Magic Select (basılı tutup blokların üstünden geç)", "key.keyboard.g"),
    ("tool", "Eksen: etkin araç (basılı tutup çiz)", "key.keyboard.b"),
    ("tool2", "Eksen: etkin aracın ikinci işlemi (sil, alçalt …)", "key.keyboard.n"),
    ("replace", "Eksen: değiştirme modu aç/kapat", "key.keyboard.r"),
    ("editor", "Eksen: editör ekranı (aç/kapat)", "key.keyboard.right.shift"),
];
/// `KEYS` index of the editor screen's key.
pub const KEY_EDITOR: usize = 8;
/// `KEYS` index of Magic Select's key.
pub const KEY_MAGIC: usize = 4;

/// The most blocks of a set selection drawn one by one; past it the set is
/// drawn as its bounding box.
pub const DRAW_MAX: usize = 2048;

/// The world's blocks, for Magic Select, the tools and undo.
///
/// Reading is the game's own: `game.world.block(x, y, z)` and
/// `game.res.facts().is_air(state)` are what BetterHitreg (render.rs) uses,
/// and `game.world.chunks` keyed by chunk (x, z) is what HatLayers checks.
/// Horizon's source is not public, so the rest of what a state says is
/// guessed in the `horizon_*` functions just below, the only lines to fix
/// if the first build disagrees.
pub struct GameBlocks<'g, 'a>(pub &'g super::Game<'a>);

impl magic::Blocks for GameBlocks<'_, '_> {
    fn block(&self, at: magic::Pos) -> Option<magic::Block> {
        let game = self.0;
        if !game.world.chunks.contains_key(&(at[0] >> 4, at[2] >> 4)) {
            return None;
        }
        let state = game.world.block(at[0], at[1], at[2]);
        if game.res.facts().is_air(state) {
            return Some(magic::Block { id: "minecraft:air".into(), state: String::new(), solid: false });
        }
        // UNVERIFIED (Horizon guesses, see below): the state as a command
        // writes it (`minecraft:oak_stairs[facing=east,half=bottom]`), and
        // whether it is solid (stone, glass panes; not flowers or water).
        let text: String = game.res.facts().state_string(state);
        let solid: bool = game.res.facts().is_solid(state);
        let (id, props) = match text.split_once('[') {
            Some((id, rest)) => (id.to_owned(), rest.trim_end_matches(']').to_owned()),
            None => (text, String::new()),
        };
        Some(magic::Block { id, state: props, solid })
    }

    fn state_id(&self, block: &str) -> Option<u32> {
        horizon_state_id(self.0, block)
    }

    fn dimension(&self) -> Option<String> {
        let d = self.0.native_player()?.dimension;
        Some(if d.contains(':') { d.to_owned() } else { format!("minecraft:{d}") })
    }
}

// ---- Horizon guesses (UNVERIFIED: Horizon's source is private) ----------
// Four calls in all: `state_string` and `is_solid` in `block` above, and
// `parse_state` / `PROTOCOL_VERSION` below. If the first build fails, these
// are the lines to fix; nothing else depends on how Horizon names them.

/// The global state id (`Block.BLOCK_STATE_REGISTRY`) of a block string,
/// as AxiomPaper reads ids; None when the game does not know it.
fn horizon_state_id(game: &super::Game<'_>, block: &str) -> Option<u32> {
    let block = if block.contains(':') { block.to_owned() } else { format!("minecraft:{block}") };
    game.res.facts().parse_state(&block).map(|s| u32::from(s.id()))
}

/// The camera the world was last drawn from: position, yaw, pitch and
/// vertical field of view (VoiceChat's world.rs reads the same field).
pub fn camera(game: &super::Game<'_>) -> Option<([f64; 3], f32, f32, f32)> {
    game.locator_camera
}

/// The block under the editor screen's pointer at `mouse` (GUI pixels on a
/// `size` screen) and the face it shows, and the ray's start and direction.
pub fn pointer(game: &super::Game<'_>, mouse: [f32; 2], size: [f32; 2]) -> Option<(Option<(Pos, Pos)>, [f64; 3], [f64; 3])> {
    let (pos, yaw, pitch, fov) = camera(game)?;
    let dir = super::eksen_ui::mouse_ray(yaw, pitch, fov, mouse, size);
    let hit = super::eksen_ui::ray(&GameBlocks(game), pos, dir, f64::from(REACH));
    Some((hit, pos, dir))
}

/// Runs `/eksen <words>` for the editor screen: the player as the chat
/// command sees them, but the block "looked at" is the one under the pointer.
pub fn run_from_screen(game: &super::Game<'_>, words: &[String]) -> Vec<String> {
    let player = game.native_player();
    let hover = state().ui.as_ref().and_then(|u| u.hover);
    let world = GameBlocks(game);
    let eye = camera(game).map(|c| c.0).or_else(|| player.as_ref().map(|p| [p.x, p.y + 1.62, p.z]));
    let ctx = Ctx {
        feet: player.as_ref().map(|p| [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32]),
        look: player.as_ref().map(|p| look_dir(p.yaw, p.pitch)),
        hit: hover.map(|h| h.0).or_else(|| game.native_raycast(REACH).map(|h| [h.block_x, h.block_y, h.block_z])),
        eye,
        view: player.as_ref().map(|p| super::eksen_editor::view(p.yaw, p.pitch)),
        world: &world,
    };
    let args: Vec<&str> = words.iter().map(String::as_str).collect();
    run(&args, &ctx)
}

/// The protocol version of the game (what the server was joined with).
pub fn protocol_version() -> i32 {
    horizon_protocol::PROTOCOL_VERSION as i32
}

/// How far the editor reaches along the view, in blocks.
pub const REACH: f32 = 256.0;
/// Vanilla's `commandModificationBlockLimit` default: the most blocks one
/// `fill` or `clone` may touch.
pub const LIMIT: i64 = 32768;
/// Commands sent per tick (a server kicks a non-operator who spams; editing
/// needs operator rights anyway, but keep the pace gentle).
pub const PER_TICK: usize = 4;
/// The most commands queued at once.
const QUEUE_MAX: usize = 20000;
/// The largest brush radius.
const BRUSH_MAX: i32 = 16;
/// The most copies `stack` makes.
const STACK_MAX: i32 = 64;

pub const PREFIX: &str = "[Eksen] ";

pub type Pos = [i32; 3];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    pub min: Pos,
    pub max: Pos,
}

impl Region {
    pub fn new(a: Pos, b: Pos) -> Self {
        Region {
            min: [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
            max: [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
        }
    }
    pub fn size(&self) -> [i64; 3] {
        [0, 1, 2].map(|i| (self.max[i] - self.min[i]) as i64 + 1)
    }
    pub fn volume(&self) -> i64 {
        let s = self.size();
        s[0] * s[1] * s[2]
    }
    /// World-space corners of the region's outer faces.
    pub fn bounds(&self) -> ([f64; 3], [f64; 3]) {
        (
            self.min.map(|v| v as f64),
            self.max.map(|v| v as f64 + 1.0),
        )
    }
    pub fn offset(&self, d: Pos) -> Region {
        Region {
            min: [0, 1, 2].map(|i| self.min[i] + d[i]),
            max: [0, 1, 2].map(|i| self.max[i] + d[i]),
        }
    }
    pub fn overlaps(&self, o: &Region) -> bool {
        (0..3).all(|i| self.min[i] <= o.max[i] && o.min[i] <= self.max[i])
    }
}

#[derive(Clone, Debug)]
pub struct Brush {
    pub radius: i32,
    pub block: String,
    pub mask: Option<String>,
}

pub struct State {
    pub pos1: Option<Pos>,
    pub pos2: Option<Pos>,
    /// A selection of single blocks (Magic Select); when there is one, the
    /// corners are unset.
    pub set: BTreeSet<Pos>,
    /// Magic Select's options; None: the defaults.
    pub magic: Option<magic::Options>,
    /// How the next selection joins this one.
    pub op: magic::Op,
    /// The blocks passed over while the Magic Select key is held.
    pub magic_seeds: Vec<Pos>,
    pub magic_held: bool,
    pub clipboard: Option<Region>,
    pub brush: Option<Brush>,
    pub brush_held: bool,
    pub last_brush: Option<Pos>,
    pub queue: VecDeque<String>,
    /// Commands of the running job, to say when it is done.
    pub job: usize,
    /// The tools, history, symmetry and capabilities (`eksen_editor`),
    /// made on first use.
    pub editor: Option<Box<super::eksen_editor::Editor>>,
    /// The server's AxiomPaper plugin (`eksen_axiom`), made on first use.
    pub axiom: Option<Box<super::eksen_axiom::Axiom>>,
    /// The editor screen (Right Shift, `eksen_ui`), made on first use.
    pub ui: Option<Box<super::eksen_ui::Ui>>,
}

static STATE: Mutex<State> = Mutex::new(State {
    editor: None,
    axiom: None,
    ui: None,
    pos1: None,
    pos2: None,
    set: BTreeSet::new(),
    magic: None,
    op: magic::Op::Add,
    magic_seeds: Vec::new(),
    magic_held: false,
    clipboard: None,
    brush: None,
    brush_held: false,
    last_brush: None,
    queue: VecDeque::new(),
    job: 0,
});

pub fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// What a command knows about the player when it runs.
pub struct Ctx<'w> {
    /// The block at the player's feet.
    pub feet: Option<Pos>,
    /// The axis the player looks along most (a unit step).
    pub look: Option<Pos>,
    /// The block the player looks at, within `REACH`.
    pub hit: Option<Pos>,
    /// The eyes and the view direction (a unit vector).
    pub eye: Option<[f64; 3]>,
    pub view: Option<[f64; 3]>,
    /// The world's blocks (see `GameBlocks`).
    pub world: &'w dyn magic::Blocks,
}

/// The axis the player looks along most: yaw 0 faces south (+z), 90 west
/// (-x); pitch above 45 looks down.
pub fn look_dir(yaw: f32, pitch: f32) -> Pos {
    if pitch > 45.0 {
        return [0, -1, 0];
    }
    if pitch < -45.0 {
        return [0, 1, 0];
    }
    let y = yaw.rem_euclid(360.0);
    if !(45.0..315.0).contains(&y) {
        [0, 0, 1]
    } else if y < 135.0 {
        [-1, 0, 0]
    } else if y < 225.0 {
        [0, 0, -1]
    } else {
        [1, 0, 0]
    }
}

fn dir_name(d: Pos) -> &'static str {
    match d {
        [1, 0, 0] => "doğu",
        [-1, 0, 0] => "batı",
        [0, 1, 0] => "yukarı",
        [0, -1, 0] => "aşağı",
        [0, 0, 1] => "güney",
        _ => "kuzey",
    }
}

fn parse_dir(word: &str) -> Option<Pos> {
    Some(match word {
        "dogu" | "doğu" | "east" | "e" => [1, 0, 0],
        "bati" | "batı" | "west" | "w" => [-1, 0, 0],
        "yukari" | "yukarı" | "up" | "u" => [0, 1, 0],
        "asagi" | "aşağı" | "down" | "d" => [0, -1, 0],
        "guney" | "güney" | "south" | "s" => [0, 0, 1],
        "kuzey" | "north" | "n" => [0, 0, -1],
        _ => return None,
    })
}

/// Splits a region into pieces of at most `LIMIT` blocks, halving the longest
/// side.
pub fn split(r: Region, out: &mut Vec<Region>) {
    if r.volume() <= LIMIT {
        out.push(r);
        return;
    }
    let s = r.size();
    let axis = (0..3).max_by_key(|&i| s[i]).unwrap_or(0);
    let mid = r.min[axis] + (s[axis] / 2) as i32 - 1;
    let mut a = r;
    a.max[axis] = mid;
    let mut b = r;
    b.min[axis] = mid + 1;
    split(a, out);
    split(b, out);
}

fn pieces(r: Region) -> Vec<Region> {
    let mut out = Vec::new();
    split(r, &mut out);
    out
}

fn fill_line(r: &Region, block: &str, tail: &str) -> String {
    format!(
        "fill {} {} {} {} {} {} {}{}",
        r.min[0], r.min[1], r.min[2], r.max[0], r.max[1], r.max[2], block, tail
    )
}

/// `fill` commands for a whole region, split to fit the limit.
fn fill_cmds(r: Region, block: &str, tail: &str, out: &mut Vec<String>) {
    for p in pieces(r) {
        out.push(fill_line(&p, block, tail));
    }
}

/// The six outer faces of a region (they overlap at the edges; filling twice
/// is harmless).
fn faces(r: Region) -> Vec<Region> {
    let mut f = Vec::new();
    for axis in 0..3 {
        let mut lo = r;
        lo.max[axis] = r.min[axis];
        let mut hi = r;
        hi.min[axis] = r.max[axis];
        f.push(lo);
        if r.max[axis] != r.min[axis] {
            f.push(hi);
        }
    }
    f
}

fn interior(r: Region) -> Option<Region> {
    let s = r.size();
    if s.iter().any(|&v| v < 3) {
        return None;
    }
    Some(Region {
        min: r.min.map(|v| v + 1),
        max: r.max.map(|v| v - 1),
    })
}

/// `clone` commands copying `src` so its min corner lands on `dest`.
fn clone_cmds(src: Region, dest: Pos, out: &mut Vec<String>) {
    let d = [0, 1, 2].map(|i| dest[i] - src.min[i]);
    let force = src.overlaps(&src.offset(d));
    let tail = if force { " replace force" } else { "" };
    for p in pieces(src) {
        let to = [0, 1, 2].map(|i| p.min[i] + d[i]);
        out.push(format!(
            "clone {} {} {} {} {} {} {} {} {}{}",
            p.min[0], p.min[1], p.min[2], p.max[0], p.max[1], p.max[2], to[0], to[1], to[2], tail
        ));
    }
}

/// Row-by-row `fill`s that make a ball of `brush` around `c`.
pub fn sphere(brush: &Brush, c: Pos, out: &mut VecDeque<String>) {
    let r = brush.radius;
    let rr = (r as f64 + 0.5) * (r as f64 + 0.5);
    let tail = brush
        .mask
        .as_ref()
        .map(|m| format!(" replace {m}"))
        .unwrap_or_default();
    for dy in -r..=r {
        for dz in -r..=r {
            let rem = rr - (dy * dy + dz * dz) as f64;
            if rem < 0.0 {
                continue;
            }
            let dx = rem.sqrt().floor() as i32;
            let row = Region {
                min: [c[0] - dx, c[1] + dy, c[2] + dz],
                max: [c[0] + dx, c[1] + dy, c[2] + dz],
            };
            if out.len() < QUEUE_MAX {
                out.push_back(fill_line(&row, &brush.block, &tail));
            }
        }
    }
}

fn fmt_pos(p: Pos) -> String {
    format!("{} {} {}", p[0], p[1], p[2])
}

fn fmt_size(r: &Region) -> String {
    let s = r.size();
    format!("{}x{}x{} = {} blok", s[0], s[1], s[2], r.volume())
}

/// A block id as a command takes it (`stone` → `minecraft:stone` is left
/// to the server); refuses words that would break the command line.
fn block_arg(word: Option<&&str>) -> Result<String, String> {
    let w = word.ok_or("Blok adı gerekli, ör. stone veya oak_planks.")?;
    if w.is_empty() || w.chars().any(|c| c.is_whitespace() || c == '/') {
        return Err(format!("Geçersiz blok: {w}"));
    }
    Ok((*w).to_owned())
}

const HELP: [&str; 23] = [
    "Eksen — 16.10.2 komutları (OP / hile izni ya da sunucuda AxiomPaper gerekir):",
    "J / K: bakılan bloğu 1. / 2. köşe yap · U: seçimi temizle · H: fırça (basılı tut)",
    "G: Magic Select — basılı tutup bloklara bak, bırakınca aynı türden bitişik bloklar seçilir",
    "/eksen sihirli — ayarları göster · karsilastir <blok|durum|kati|herhangi> · limit <n> · menzil <1-8>",
    "/eksen sihirli yuzey|yukari|asagi|yatay|koseler — aç/kapat · /eksen islem <ekle|cikar|yerine|kesisim>",
    "/eksen pos1 | pos2 — ayağının altındaki blok",
    "/eksen doldur <blok> — seçimi doldur",
    "/eksen degistir <eski> <yeni> — seçimde bloğu değiştir",
    "/eksen bos <blok> — içi boş kutu · /eksen dis <blok> — sadece dış yüzey",
    "/eksen duvar <blok> — dört duvar · /eksen sil — seçimi havaya çevir",
    "/eksen kopyala — seçimi panoya al",
    "/eksen yapistir — panoyu baktığın bloğun üstüne yapıştır",
    "/eksen cogalt <adet> [yön] — seçimi baktığın yöne çoğalt",
    "/eksen kaydir <adet> [yön] — seçimi taşı (eskisi havaya döner)",
    "/eksen firca <yarıçap> <blok> [sadece <blok>] · /eksen firca kapat",
    "/eksen secim — seçimi göster · /eksen durdur — kuyruğu boşalt",
    "Araçlar: /eksen arac [ad] · /eksen ayar <ayar> [değer] · /eksen blok <blok> · B basılı tut: araç, N: ikinci işlem",
    "Yapı araçları: tasi|klon <x y z> [adet] · sur <adet> · uzat|kisalt · silbagli · cevir · aynala · bayrak koru|hava",
    "Simetri: /eksen simetri nokta|merkez|aynala|cevir|kaldir · Geri al: Ctrl+Z / Ctrl+Y (/eksen geri, ileri)",
    "Yetenekler: /eksen yetenek [ad] (R: değiştirme modu) · uçuş ve araç kutusu: /eksen ucus <hiz|yon|ivme|menzil|sivi|parlaklik> <değer>",
    "Sağ Shift: editör ekranı — fare serbest, solda araçlar/yapı/yetenekler/seçim, sağda aracın ayarları, ortada tıkla: aracı kullan",
    "/eksen axiom [ac|kapat|surum <protokol>] — sunucuda AxiomPaper varsa düzenlemeler eklentinin yolundan gider (OP gerekmez)",
    "Tuşlar Options > Mods > Eksen'den değiştirilebilir. Geri alma yalnızca araçların değişikliklerinde çalışır.",
];

impl State {
    fn selection(&self) -> Result<Region, String> {
        if !self.set.is_empty() {
            return Err("Bu işlem kutu seçimi ister: Magic Select seçimi için doldur, sil veya degistir kullan.".into());
        }
        match (self.pos1, self.pos2) {
            (Some(a), Some(b)) => Ok(Region::new(a, b)),
            _ => Err("Önce iki köşe seç (J ve K, ya da /eksen pos1, pos2).".into()),
        }
    }

    /// What a fill covers: the set's rows, or the box split to fit the limit.
    fn targets(&self) -> Result<Vec<Region>, String> {
        if self.set.is_empty() {
            return Ok(pieces(self.selection()?));
        }
        Ok(magic::runs(&self.set).into_iter().map(|(a, b)| Region::new(a, b)).collect())
    }

    /// A corner set by key or command: a box selection replaces a set.
    pub fn set_corner(&mut self, which: u8, p: Pos) {
        self.set.clear();
        if which == 1 {
            self.pos1 = Some(p);
        } else {
            self.pos2 = Some(p);
        }
    }

    pub fn clear_selection(&mut self) {
        self.pos1 = None;
        self.pos2 = None;
        self.set.clear();
    }

    /// The Magic Select key released: select from the blocks passed over and
    /// join the result by the current operation; the chat line to show.
    pub fn finish_magic(&mut self, world: &dyn magic::Blocks) -> String {
        self.magic_held = false;
        let seeds = std::mem::take(&mut self.magic_seeds);
        if seeds.is_empty() {
            return format!("{PREFIX}Menzilde blok yok.");
        }
        if world.block(seeds[0]).is_none() {
            return format!("{PREFIX}Magic Select bu Horizon sürümünde blokları okuyamıyor (dünya okuma bağlantısı henüz yok).");
        }
        let options = self.magic.clone().unwrap_or_default();
        let found = magic::select(world, &seeds, &options);
        let n = found.len();
        if self.set.is_empty() {
            // A box selection is not mixed with a set: the set starts anew.
            self.pos1 = None;
            self.pos2 = None;
        }
        self.op.apply(&mut self.set, found);
        let cut = if n >= options.limit { " (limite ulaşıldı)" } else { "" };
        format!("{PREFIX}Magic Select: {n} blok{cut} · {} · seçim {} blok", self.op.name(), self.set.len())
    }

    fn magic_lines(&self) -> Vec<String> {
        let o = self.magic.clone().unwrap_or_default();
        let on = |b: bool| if b { "açık" } else { "kapalı" };
        vec![
            format!("{PREFIX}Magic Select ayarları · işlem: {}", self.op.name()),
            format!(
                "karşılaştır {} · limit {} · menzil {} · yüzey {} · yukarı {} · aşağı {} · yatay {} · köşeler {}",
                o.compare.name(),
                o.limit,
                o.range,
                on(o.surface_only),
                on(o.up),
                on(o.down),
                on(o.horizontal),
                on(o.corners)
            ),
        ]
    }

    /// `/eksen sihirli <option> [value]`.
    fn magic_option(&mut self, args: &[&str]) -> Result<Vec<String>, String> {
        let mut o = self.magic.clone().unwrap_or_default();
        match args.first().copied() {
            None => return Ok(self.magic_lines()),
            Some("karsilastir" | "karşılaştır" | "compare") => {
                let w = args.get(1).ok_or("Mod gerekli: blok, durum, kati veya herhangi.")?;
                o.compare = magic::Compare::parse(w).ok_or_else(|| format!("Bilinmeyen mod: {w}"))?;
            }
            Some("limit") => {
                o.limit = args
                    .get(1)
                    .and_then(|w| w.parse().ok())
                    .filter(|n| *n > 0)
                    .ok_or("Limit pozitif bir sayı olmalı, ör. /eksen sihirli limit 100000")?;
            }
            Some("menzil" | "range") => {
                o.range = args
                    .get(1)
                    .and_then(|w| w.parse().ok())
                    .filter(|n| (1..=magic::RANGE_MAX).contains(n))
                    .ok_or(format!("Menzil 1 ile {} arasında olmalı.", magic::RANGE_MAX))?;
            }
            Some(w @ ("yuzey" | "yüzey" | "surface" | "yukari" | "yukarı" | "up" | "asagi" | "aşağı" | "down" | "yatay" | "horizontal" | "koseler" | "köşeler" | "corners")) => {
                let flag = match w {
                    "yuzey" | "yüzey" | "surface" => &mut o.surface_only,
                    "yukari" | "yukarı" | "up" => &mut o.up,
                    "asagi" | "aşağı" | "down" => &mut o.down,
                    "yatay" | "horizontal" => &mut o.horizontal,
                    _ => &mut o.corners,
                };
                *flag = !*flag;
                if o.corners && !o.horizontal {
                    o.corners = false;
                }
            }
            Some("sifirla" | "sıfırla" | "reset") => o = magic::Options::default(),
            Some(w) => return Err(format!("Bilinmeyen ayar: {w}. /eksen yardim")),
        }
        self.magic = Some(o);
        Ok(self.magic_lines())
    }

    /// The editor, made on first use.
    pub fn editor(&mut self) -> &mut super::eksen_editor::Editor {
        self.editor.get_or_insert_with(Default::default)
    }

    /// The AxiomPaper link, made on first use.
    pub fn axiom(&mut self) -> &mut super::eksen_axiom::Axiom {
        self.axiom.get_or_insert_with(Default::default)
    }

    /// The editor screen, made on first use.
    pub fn ui(&mut self) -> &mut super::eksen_ui::Ui {
        self.ui.get_or_insert_with(Default::default)
    }

    pub fn enqueue(&mut self, cmds: Vec<String>) -> Result<usize, String> {
        if self.queue.len() + cmds.len() > QUEUE_MAX {
            return Err(format!(
                "Çok büyük: {} komut gerekir (sınır {QUEUE_MAX}). Seçimi küçült.",
                cmds.len()
            ));
        }
        let n = cmds.len();
        self.job += n;
        self.queue.extend(cmds);
        Ok(n)
    }

    pub fn describe(&self, which: u8, p: Pos) -> String {
        let mut s = format!("{PREFIX}{which}. köşe: {}", fmt_pos(p));
        if let Ok(r) = self.selection() {
            s.push_str(&format!(" · seçim {}", fmt_size(&r)));
        }
        s
    }

    /// Lines for the HUD box; empty when the editor is idle.
    pub fn hud_lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.set.is_empty() {
            out.push(format!("Seçim: {} blok (Magic Select)", self.set.len()));
        }
        if self.magic_held {
            out.push(format!("Magic Select: {} blok türü", self.magic_seeds.len()));
        }
        if let Some(e) = &self.editor {
            out.push(format!("Araç: {} · blok {}", e.tool_name(), e.active.trim_start_matches("minecraft:")));
            if e.held {
                out.push(format!("Çizim: {} blok", e.stroke.len()));
            }
            if e.symmetry.on() {
                out.push("Simetri: açık".into());
            }
            let speed = e.abilities.flight_speed;
            if speed != 100 {
                out.push(format!("Uçuş hızı: %{speed}"));
            }
        }
        match (self.pos1, self.pos2) {
            (Some(a), Some(b)) => out.push(format!("Seçim: {}", fmt_size(&Region::new(a, b)))),
            (Some(_), None) | (None, Some(_)) => out.push("Seçim: bir köşe daha".into()),
            _ => {}
        }
        if let Some(b) = &self.brush {
            let mask = b.mask.as_ref().map(|m| format!(" (sadece {m})")).unwrap_or_default();
            out.push(format!("Fırça: r{} {}{}", b.radius, b.block, mask));
        }
        if self.clipboard.is_some() {
            out.push("Pano: dolu".into());
        }
        if !self.queue.is_empty() {
            out.push(format!("Kuyruk: {} komut", self.queue.len()));
        }
        if !out.is_empty() {
            out.insert(0, format!("{NAME} — {VERSION}"));
        }
        out
    }
}

/// Runs `/eksen <args>`: changes the state, queues commands, and returns the
/// chat lines to show.
pub fn run(args: &[&str], ctx: &Ctx) -> Vec<String> {
    let mut st = state();
    match run_in(&mut st, args, ctx) {
        Ok(lines) => lines,
        Err(e) => vec![format!("{PREFIX}{e}")],
    }
}

fn count_arg(word: Option<&&str>) -> Result<i32, String> {
    let n: i32 = word
        .ok_or("Adet gerekli, ör. /eksen cogalt 3")?
        .parse()
        .map_err(|_| "Adet bir sayı olmalı.".to_string())?;
    if !(1..=STACK_MAX).contains(&n) {
        return Err(format!("Adet 1 ile {STACK_MAX} arasında olmalı."));
    }
    Ok(n)
}

fn dir_arg(word: Option<&&str>, ctx: &Ctx) -> Result<Pos, String> {
    match word {
        Some(w) => parse_dir(w).ok_or_else(|| format!("Bilinmeyen yön: {w}")),
        None => ctx.look.ok_or_else(|| "Yön bulunamadı.".into()),
    }
}

fn queued(n: usize, what: &str) -> Vec<String> {
    vec![format!("{PREFIX}{what}: {n} komut sıraya alındı.")]
}

fn run_in(st: &mut State, args: &[&str], ctx: &Ctx) -> Result<Vec<String>, String> {
    let sub = args.first().copied().unwrap_or("yardim");
    match sub {
        "yardim" | "help" | "?" => Ok(HELP.iter().map(|l| l.to_string()).collect()),
        "pos1" | "pos2" => {
            let p = ctx.feet.ok_or("Oyuncu bulunamadı.")?;
            let which = if sub == "pos1" { 1 } else { 2 };
            st.set_corner(which, p);
            Ok(vec![st.describe(which, p)])
        }
        "sihirli" | "magic" => st.magic_option(&args[1..]),
        "islem" | "işlem" | "op" => {
            let w = args.get(1).ok_or("İşlem gerekli: ekle, cikar, yerine veya kesisim.")?;
            st.op = magic::Op::parse(w).ok_or_else(|| format!("Bilinmeyen işlem: {w}"))?;
            Ok(vec![format!("{PREFIX}Sonraki seçim işlemi: {}.", st.op.name())])
        }
        "secim" | "sel" if !st.set.is_empty() => {
            let (lo, hi) = magic::bounds(&st.set).ok_or("Seçim boş.")?;
            Ok(vec![format!("{PREFIX}{} blok, {} → {} kutusunda", st.set.len(), fmt_pos(lo), fmt_pos(hi))])
        }
        "secim" | "sel" => {
            let r = st.selection()?;
            Ok(vec![format!(
                "{PREFIX}{} → {} · {}",
                fmt_pos(r.min),
                fmt_pos(r.max),
                fmt_size(&r)
            )])
        }
        "temizle" | "clear" => {
            st.clear_selection();
            Ok(vec![format!("{PREFIX}Seçim temizlendi.")])
        }
        "durdur" | "stop" => {
            let n = st.queue.len();
            st.queue.clear();
            st.job = 0;
            Ok(vec![format!("{PREFIX}Kuyruk boşaltıldı ({n} komut iptal).")])
        }
        "doldur" | "fill" => {
            let block = block_arg(args.get(1))?;
            let c = st.targets()?.iter().map(|r| fill_line(r, &block, "")).collect();
            Ok(queued(st.enqueue(c)?, "Doldur"))
        }
        "sil" | "air" => {
            let c = st.targets()?.iter().map(|r| fill_line(r, "air", "")).collect();
            Ok(queued(st.enqueue(c)?, "Sil"))
        }
        "degistir" | "replace" => {
            let from = block_arg(args.get(1))?;
            let to = block_arg(args.get(2))?;
            let tail = format!(" replace {from}");
            let c = st.targets()?.iter().map(|r| fill_line(r, &to, &tail)).collect();
            Ok(queued(st.enqueue(c)?, "Değiştir"))
        }
        "dis" | "outline" | "bos" | "hollow" => {
            let r = st.selection()?;
            let block = block_arg(args.get(1))?;
            let mut c = Vec::new();
            for f in faces(r) {
                fill_cmds(f, &block, "", &mut c);
            }
            if matches!(sub, "bos" | "hollow") {
                if let Some(i) = interior(r) {
                    fill_cmds(i, "air", "", &mut c);
                }
            }
            Ok(queued(st.enqueue(c)?, "Kutu"))
        }
        "duvar" | "walls" => {
            let r = st.selection()?;
            let block = block_arg(args.get(1))?;
            let mut c = Vec::new();
            for axis in [0usize, 2] {
                let mut lo = r;
                lo.max[axis] = r.min[axis];
                let mut hi = r;
                hi.min[axis] = r.max[axis];
                fill_cmds(lo, &block, "", &mut c);
                fill_cmds(hi, &block, "", &mut c);
            }
            Ok(queued(st.enqueue(c)?, "Duvar"))
        }
        "kopyala" | "copy" => {
            let r = st.selection()?;
            st.clipboard = Some(r);
            Ok(vec![format!(
                "{PREFIX}Panoya alındı: {}. Kaynak bölgeyi yapıştırmadan önce değiştirme (yapıştırma oradan kopyalar).",
                fmt_size(&r)
            )])
        }
        "yapistir" | "paste" => {
            let src = st.clipboard.ok_or("Pano boş: önce /eksen kopyala.")?;
            let hit = ctx.hit.ok_or("Yapıştırmak için bir bloğa bak.")?;
            let dest = [hit[0], hit[1] + 1, hit[2]];
            let mut c = Vec::new();
            clone_cmds(src, dest, &mut c);
            let n = st.enqueue(c)?;
            let placed = src.offset([0, 1, 2].map(|i| dest[i] - src.min[i]));
            st.pos1 = Some(placed.min);
            st.pos2 = Some(placed.max);
            Ok(vec![format!(
                "{PREFIX}Yapıştır: {} konumuna, {n} komut. Seçim yeni kopyaya taşındı.",
                fmt_pos(dest)
            )])
        }
        "cogalt" | "stack" => {
            let r = st.selection()?;
            let n = count_arg(args.get(1))?;
            let d = dir_arg(args.get(2), ctx)?;
            let s = r.size();
            let step = [0, 1, 2].map(|i| d[i] * s[i] as i32);
            let mut c = Vec::new();
            for k in 1..=n {
                let to = [0, 1, 2].map(|i| r.min[i] + step[i] * k);
                clone_cmds(r, to, &mut c);
            }
            let m = st.enqueue(c)?;
            Ok(vec![format!("{PREFIX}Çoğalt: {n} kopya {} yönüne, {m} komut.", dir_name(d))])
        }
        "kaydir" | "move" => {
            let r = st.selection()?;
            let n = count_arg(args.get(1))?;
            let d = dir_arg(args.get(2), ctx)?;
            let shift = d.map(|v| v * n);
            let dest = r.offset(shift);
            let mut c = Vec::new();
            if r.overlaps(&dest) {
                return Err("Kaydırma, eski ve yeni yer çakışınca desteklenmiyor: adedi seçim boyundan büyük yap.".into());
            }
            clone_cmds(r, dest.min, &mut c);
            fill_cmds(r, "air", "", &mut c);
            let m = st.enqueue(c)?;
            st.pos1 = Some(dest.min);
            st.pos2 = Some(dest.max);
            Ok(vec![format!("{PREFIX}Kaydır: {n} blok {} yönüne, {m} komut.", dir_name(d))])
        }
        "firca" | "brush" => {
            match args.get(1).copied() {
                None | Some("kapat") | Some("off") => {
                    st.brush = None;
                    st.brush_held = false;
                    return Ok(vec![format!("{PREFIX}Fırça kapatıldı.")]);
                }
                _ => {}
            }
            let radius: i32 = args[1]
                .parse()
                .map_err(|_| "Yarıçap bir sayı olmalı, ör. /eksen firca 3 stone".to_string())?;
            if !(0..=BRUSH_MAX).contains(&radius) {
                return Err(format!("Yarıçap 0 ile {BRUSH_MAX} arasında olmalı."));
            }
            let block = block_arg(args.get(2))?;
            let mask = match args.get(3).copied() {
                Some("sadece") | Some("only") => Some(block_arg(args.get(4))?),
                None => None,
                Some(w) => return Err(format!("Bilinmeyen seçenek: {w} (sadece <blok>)")),
            };
            st.brush = Some(Brush { radius, block, mask });
            Ok(vec![format!(
                "{PREFIX}Fırça hazır: yarıçap {radius}. H'yi basılı tutup bloklara bak."
            )])
        }
        other => super::eksen_editor::command(st, args, ctx)
            .unwrap_or_else(|| Err(format!("Bilinmeyen komut: {other}. /eksen yardim"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_fits_limit() {
        let r = Region::new([0, 0, 0], [99, 99, 99]);
        let p = pieces(r);
        assert!(p.iter().all(|x| x.volume() <= LIMIT));
        assert_eq!(p.iter().map(|x| x.volume()).sum::<i64>(), r.volume());
    }

    #[test]
    fn look() {
        assert_eq!(look_dir(0.0, 0.0), [0, 0, 1]);
        assert_eq!(look_dir(90.0, 0.0), [-1, 0, 0]);
        assert_eq!(look_dir(-90.0, 0.0), [1, 0, 0]);
        assert_eq!(look_dir(180.0, 0.0), [0, 0, -1]);
        assert_eq!(look_dir(0.0, 80.0), [0, -1, 0]);
    }
}
