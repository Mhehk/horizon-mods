// Eksen — mhv1 (mihehk): a building editor for Horizon. This module holds the
// selection, the clipboard, the brush and the command queue, and turns editor
// actions into vanilla commands (`fill`, `clone`) the server runs. The hooks
// in this bundle only read and write this state and call the game's helpers.
use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};

pub const ID: &str = "eksen";
pub const NAME: &str = "Eksen";
pub const VERSION: &str = "mhv1";
pub const DESCRIPTION: &str = "Yapı editörü: uzaktan seçim, doldur, değiştir, kopyala/yapıştır, çoğalt, fırça. /eksen yardim";

/// Mod keys: name, label, default key.
pub const KEYS: [(&str, &str, &str); 4] = [
    ("pos1", "Eksen: 1. köşe (bakılan blok)", "key.keyboard.j"),
    ("pos2", "Eksen: 2. köşe (bakılan blok)", "key.keyboard.k"),
    ("clear", "Eksen: seçimi temizle", "key.keyboard.u"),
    ("brush", "Eksen: fırça (basılı tut)", "key.keyboard.h"),
];

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
    pub clipboard: Option<Region>,
    pub brush: Option<Brush>,
    pub brush_held: bool,
    pub last_brush: Option<Pos>,
    pub queue: VecDeque<String>,
    /// Commands of the running job, to say when it is done.
    pub job: usize,
}

static STATE: Mutex<State> = Mutex::new(State {
    pos1: None,
    pos2: None,
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
pub struct Ctx {
    /// The block at the player's feet.
    pub feet: Option<Pos>,
    /// The axis the player looks along most (a unit step).
    pub look: Option<Pos>,
    /// The block the player looks at, within `REACH`.
    pub hit: Option<Pos>,
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

const HELP: [&str; 14] = [
    "Eksen — mhv1 komutları (OP / hile izni gerekir):",
    "J / K: bakılan bloğu 1. / 2. köşe yap · U: seçimi temizle · H: fırça (basılı tut)",
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
    "Tuşlar Options > Mods > Eksen'den değiştirilebilir. Geri alma yok: önce yedek al.",
];

impl State {
    fn selection(&self) -> Result<Region, String> {
        match (self.pos1, self.pos2) {
            (Some(a), Some(b)) => Ok(Region::new(a, b)),
            _ => Err("Önce iki köşe seç (J ve K, ya da /eksen pos1, pos2).".into()),
        }
    }

    fn enqueue(&mut self, cmds: Vec<String>) -> Result<usize, String> {
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
            if which == 1 {
                st.pos1 = Some(p);
            } else {
                st.pos2 = Some(p);
            }
            Ok(vec![st.describe(which, p)])
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
            st.pos1 = None;
            st.pos2 = None;
            Ok(vec![format!("{PREFIX}Seçim temizlendi.")])
        }
        "durdur" | "stop" => {
            let n = st.queue.len();
            st.queue.clear();
            st.job = 0;
            Ok(vec![format!("{PREFIX}Kuyruk boşaltıldı ({n} komut iptal).")])
        }
        "doldur" | "fill" => {
            let r = st.selection()?;
            let block = block_arg(args.get(1))?;
            let mut c = Vec::new();
            fill_cmds(r, &block, "", &mut c);
            Ok(queued(st.enqueue(c)?, "Doldur"))
        }
        "sil" | "air" => {
            let r = st.selection()?;
            let mut c = Vec::new();
            fill_cmds(r, "air", "", &mut c);
            Ok(queued(st.enqueue(c)?, "Sil"))
        }
        "degistir" | "replace" => {
            let r = st.selection()?;
            let from = block_arg(args.get(1))?;
            let to = block_arg(args.get(2))?;
            let mut c = Vec::new();
            fill_cmds(r, &to, &format!(" replace {from}"), &mut c);
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
        "geri" | "undo" => Err(
            "Geri alma bu sürümde yok. Büyük işlerden önce dünyanın yedeğini al.".into(),
        ),
        other => Err(format!("Bilinmeyen komut: {other}. /eksen yardim")),
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
