// Arena HUD — 16.10.2 (mihehk): the PvP HUD modules, plain Rust. The module
// list is Dawn Client's (dawn.gg/pvp-client): Keystrokes, CPS, Mousestrokes,
// Reach Display, Combo Display, Hitbox, Toggle Sprint, Armor Status, Potion
// Effects, Ping, FPS, Block Overlay; and seven of ours on top: Target HUD,
// Speed, Coordinates, Session Stats, Server Address, Session Time, Reach
// History. Their options follow what Lunar
// Client's Keystrokes article documents and the usual PvP client layout;
// Dawn's own settings screens are not public, so where they differ the
// options here are ours. No code, names or images of Dawn, Feather or Lunar.
//
// The hooks fill a `Snapshot` each frame; each module turns it and its
// options into draws. Counting (clicks, frames, reach, combo) is here too.
use std::collections::{BTreeMap, VecDeque};

/// Something to draw, in GUI pixels: a fill ([x, y, w, h], ARGB) or a text
/// (at, RGB, shadow).
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    Fill([f32; 4], u32),
    Text(String, [f32; 2], u32, bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Module {
    Keystrokes,
    Cps,
    Mousestrokes,
    Reach,
    Combo,
    Hitbox,
    ToggleSprint,
    ArmorStatus,
    PotionEffects,
    Ping,
    Fps,
    BlockOverlay,
    TargetHud,
    Speed,
    Coordinates,
    Stats,
    ServerAddress,
    SessionTime,
    ReachHistory,
}

pub const ALL: [Module; 19] = [
    Module::Keystrokes,
    Module::Cps,
    Module::Mousestrokes,
    Module::Reach,
    Module::Combo,
    Module::Hitbox,
    Module::ToggleSprint,
    Module::ArmorStatus,
    Module::PotionEffects,
    Module::Ping,
    Module::Fps,
    Module::BlockOverlay,
    Module::TargetHud,
    Module::Speed,
    Module::Coordinates,
    Module::Stats,
    Module::ServerAddress,
    Module::SessionTime,
    Module::ReachHistory,
];

impl Module {
    /// The word that names it in settings and `/arena`.
    pub fn id(self) -> &'static str {
        match self {
            Module::Keystrokes => "keystrokes",
            Module::Cps => "cps",
            Module::Mousestrokes => "mousestrokes",
            Module::Reach => "reach",
            Module::Combo => "combo",
            Module::Hitbox => "hitbox",
            Module::ToggleSprint => "togglesprint",
            Module::ArmorStatus => "armor",
            Module::PotionEffects => "potions",
            Module::Ping => "ping",
            Module::Fps => "fps",
            Module::BlockOverlay => "blockoverlay",
            Module::TargetHud => "target",
            Module::Speed => "speed",
            Module::Coordinates => "coords",
            Module::Stats => "stats",
            Module::ServerAddress => "server",
            Module::SessionTime => "session",
            Module::ReachHistory => "reachhistory",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Module::Keystrokes => "Keystrokes",
            Module::Cps => "CPS",
            Module::Mousestrokes => "Mousestrokes",
            Module::Reach => "Reach Display",
            Module::Combo => "Combo Display",
            Module::Hitbox => "Hitbox",
            Module::ToggleSprint => "Toggle Sprint",
            Module::ArmorStatus => "Armor Status",
            Module::PotionEffects => "Potion Effects",
            Module::Ping => "Ping",
            Module::Fps => "FPS",
            Module::BlockOverlay => "Block Overlay",
            Module::TargetHud => "Target HUD",
            Module::Speed => "Speed",
            Module::Coordinates => "Coordinates",
            Module::Stats => "Session Stats",
            Module::ServerAddress => "Server Address",
            Module::SessionTime => "Session Time",
            Module::ReachHistory => "Reach History",
        }
    }

    pub fn parse(word: &str) -> Option<Module> {
        ALL.into_iter().find(|m| m.id() == word.to_ascii_lowercase())
    }

    /// Drawn on the HUD (and moved in the editor); Hitbox and Block Overlay
    /// draw in the world instead.
    pub fn on_hud(self) -> bool {
        !matches!(self, Module::Hitbox | Module::BlockOverlay)
    }

    /// Where it sits at first: fractions of the free space, as `Layout`.
    pub fn default_place(self) -> [f32; 2] {
        match self {
            Module::Keystrokes => [0.01, 0.30],
            Module::Cps => [0.01, 0.62],
            Module::Mousestrokes => [0.01, 0.75],
            Module::Reach => [0.5, 0.62],
            Module::Combo => [0.5, 0.68],
            Module::ToggleSprint => [0.01, 0.97],
            Module::ArmorStatus => [0.99, 0.55],
            Module::PotionEffects => [0.99, 0.15],
            Module::Ping => [0.01, 0.06],
            Module::Fps => [0.01, 0.01],
            Module::TargetHud => [0.62, 0.45],
            Module::Speed => [0.01, 0.11],
            Module::Coordinates => [0.01, 0.16],
            Module::Stats => [0.99, 0.97],
            Module::ServerAddress => [0.5, 0.01],
            Module::SessionTime => [0.5, 0.05],
            Module::ReachHistory => [0.5, 0.75],
            Module::Hitbox | Module::BlockOverlay => [0.0, 0.0],
        }
    }

    /// Its own options: key, default, what it does (shown by `/arena`).
    pub fn options(self) -> &'static [(&'static str, &'static str, &'static str)] {
        match self {
            Module::Keystrokes => &[
                ("show_movement", "true", "WASD tuşları"),
                ("show_mouse", "true", "LMB / RMB kutuları"),
                ("show_cps", "true", "fare kutularında CPS"),
                ("show_space", "true", "en altta boşluk çubuğu"),
                ("arrows", "false", "WASD yerine oklar"),
                ("fade", "100", "basınca renk geçişi (ms)"),
                ("box", "18", "kutu boyutu (px)"),
                ("pressed_bg", "B4FFFFFF", "basılı kutu rengi (ARGB)"),
                ("pressed_text", "000000", "basılı yazı rengi (RGB)"),
            ],
            Module::Cps => &[("buttons", "both", "left, right, both"), ("label", "CPS", "yazının sonu")],
            Module::Mousestrokes => &[
                ("size", "40", "kutu boyutu (px)"),
                ("sensitivity", "1.0", "hareketin büyütülmesi"),
                ("dot", "FFFFFF", "nokta rengi (RGB)"),
            ],
            Module::Reach => &[("decimals", "2", "ondalık basamak"), ("timeout", "3000", "gösterme süresi (ms)"), ("label", "blocks", "birim")],
            Module::Combo => &[("timeout", "2000", "kombo bu kadar ms vuruşsuz biter"), ("hide_zero", "true", "kombo yokken gizle"), ("label", "Combo", "yazı")],
            Module::Hitbox => &[
                ("color", "FFFFFFFF", "çerçeve rengi (ARGB)"),
                ("width", "1.5", "çizgi kalınlığı"),
                ("players_only", "false", "yalnızca oyuncular"),
                ("eye_line", "true", "göz hizası çizgisi (kırmızı)"),
                ("look", "true", "bakış yönü (mavi)"),
                ("range", "64", "en uzak (blok)"),
            ],
            Module::ToggleSprint => &[("sprint", "true", "koşmayı sabitle (deneysel)"), ("show_text", "true", "durumu yaz")],
            Module::ArmorStatus => &[("hand", "true", "eldeki eşyayı da göster"), ("vertical", "true", "alt alta")],
            Module::PotionEffects => &[("vertical", "true", "alt alta")],
            Module::Ping => &[("label", "ms", "birim")],
            Module::Fps => &[("label", "FPS", "yazı")],
            Module::BlockOverlay => &[
                ("outline", "true", "çerçeve"),
                ("outline_color", "FF000000", "çerçeve rengi (ARGB)"),
                ("width", "2.0", "çerçeve kalınlığı"),
                ("fill", "false", "yüzleri boya"),
                ("fill_color", "40FFFFFF", "dolgu rengi (ARGB)"),
            ],
            Module::TargetHud => &[
                ("timeout", "3000", "son vuruştan sonra gösterme süresi (ms)"),
                ("health_bar", "true", "can çubuğu"),
                ("show_hand", "true", "rakibin elindeki eşya"),
                ("show_distance", "true", "rakibe uzaklık"),
                ("show_armor", "true", "zırh puanı"),
            ],
            Module::Speed => &[("decimals", "2", "ondalık basamak"), ("vertical", "false", "dikey hızı da say"), ("label", "BPS", "birim")],
            Module::Coordinates => &[
                ("decimals", "0", "ondalık basamak"),
                ("direction", "true", "baktığın yön"),
                ("other_dimension", "true", "Nether / Overworld karşılığı"),
            ],
            Module::Stats => &[("show_hits", "true", "toplam vuruş"), ("show_best", "true", "en uzun kombo"), ("show_average", "true", "ortalama reach")],
            Module::ServerAddress => &[("show_port", "false", "portu da yaz (:25565)"), ("label", "", "önüne yazı, ör. IP:")],
            Module::SessionTime => &[("label", "", "önüne yazı, ör. Süre:")],
            Module::ReachHistory => &[("count", "5", "kaç vuruş (1-10)"), ("decimals", "2", "ondalık basamak"), ("vertical", "false", "alt alta")],
        }
    }
}

/// Options every HUD module has.
pub const COMMON: [(&str, &str, &str); 5] = [
    ("enabled", "false", "açık"),
    ("background", "true", "arka plan kutusu"),
    ("bg_color", "6F000000", "arka plan rengi (ARGB)"),
    ("text_color", "FFFFFF", "yazı rengi (RGB)"),
    ("brackets", "false", "yazıyı [ ] içine al"),
];

/// Every setting, read through `get` (the mod's stored settings) with the
/// defaults behind it.
pub struct Options<'a> {
    pub module: Module,
    pub get: &'a dyn Fn(&str) -> Option<String>,
}

impl Options<'_> {
    pub fn key(module: Module, option: &str) -> String {
        format!("{}.{option}", module.id())
    }

    pub fn default(module: Module, option: &str) -> Option<&'static str> {
        COMMON.iter().chain(module.options()).find(|o| o.0 == option).map(|o| o.1)
    }

    pub fn text(&self, option: &str) -> String {
        (self.get)(&Self::key(self.module, option))
            .or_else(|| Self::default(self.module, option).map(str::to_owned))
            .unwrap_or_default()
    }

    pub fn on(&self, option: &str) -> bool {
        self.text(option) == "true"
    }

    pub fn number(&self, option: &str) -> f64 {
        let d = Self::default(self.module, option).and_then(|d| d.parse().ok()).unwrap_or(0.0);
        self.text(option).parse().unwrap_or(d)
    }

    pub fn color(&self, option: &str) -> u32 {
        let d = Self::default(self.module, option).and_then(|d| u32::from_str_radix(d, 16).ok()).unwrap_or(0xFFFFFF);
        u32::from_str_radix(self.text(option).trim_start_matches('#'), 16).unwrap_or(d)
    }
}

/// A value to store for `option`, checked against its default's kind;
/// Err says why not.
pub fn checked(module: Module, option: &str, value: &str) -> Result<String, String> {
    let Some(default) = Options::default(module, option) else {
        let names: Vec<&str> = COMMON.iter().chain(module.options()).map(|o| o.0).collect();
        return Err(format!("{} için böyle bir ayar yok. Ayarlar: {}", module.name(), names.join(", ")));
    };
    let value = value.trim().trim_start_matches('#');
    if default == "true" || default == "false" {
        return match value {
            "true" | "acik" | "açık" | "on" => Ok("true".into()),
            "false" | "kapali" | "kapalı" | "off" => Ok("false".into()),
            _ => Err("true ya da false olmalı.".into()),
        };
    }
    if default.len() >= 6 && default.chars().all(|c| c.is_ascii_hexdigit()) {
        return if value.len() == default.len() && value.chars().all(|c| c.is_ascii_hexdigit()) {
            Ok(value.to_ascii_uppercase())
        } else {
            Err(format!("{} haneli bir renk olmalı, ör. {default}.", default.len()))
        };
    }
    if default.parse::<f64>().is_ok() {
        return match value.parse::<f64>() {
            Ok(v) if v.is_finite() && v >= 0.0 => Ok(value.to_owned()),
            _ => Err("pozitif bir sayı olmalı.".into()),
        };
    }
    if module == Module::Cps && option == "buttons" && !matches!(value, "left" | "right" | "both") {
        return Err("left, right ya da both olmalı.".into());
    }
    Ok(value.to_owned())
}

// ---------------------------------------------------------------- counting

/// Clicks in the last second (`CPS`): a press counts for 1000 ms.
#[derive(Default)]
pub struct Clicks(VecDeque<u64>);

impl Clicks {
    pub fn press(&mut self, now: u64) {
        self.0.push_back(now);
        self.trim(now);
    }
    pub fn per_second(&mut self, now: u64) -> u32 {
        self.trim(now);
        self.0.len() as u32
    }
    fn trim(&mut self, now: u64) {
        while self.0.front().is_some_and(|t| now.saturating_sub(*t) >= 1000) {
            self.0.pop_front();
        }
    }
}

/// Frames in the last second.
#[derive(Default)]
pub struct Frames(VecDeque<u64>);

impl Frames {
    pub fn frame(&mut self, now: u64) -> u32 {
        self.0.push_back(now);
        while self.0.front().is_some_and(|t| now.saturating_sub(*t) >= 1000) {
            self.0.pop_front();
        }
        self.0.len() as u32
    }
}

/// The distance from the eyes to the nearest point of a box (min, max): how
/// reach is measured, as the server checks an attack.
pub fn reach(eye: [f64; 3], lo: [f64; 3], hi: [f64; 3]) -> f64 {
    let d: Vec<f64> = (0..3).map(|i| (lo[i] - eye[i]).max(0.0).max(eye[i] - hi[i])).collect();
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// Reach and combo from attacks and damage events. An attack is a hit when
/// its target takes damage within `CONFIRM` ms; a combo is hits in a row
/// with none more than the timeout apart, and ends when you take damage.
#[derive(Default)]
pub struct Fight {
    /// The last attack: target, its reach, when.
    pending: Option<(i64, f64, u64)>,
    /// The last hit's reach and when.
    pub last_reach: Option<(f64, u64)>,
    pub combo: u32,
    last_hit: Option<u64>,
    /// The last entity hit and when (Target HUD).
    pub target: Option<(i64, u64)>,
    /// Session Stats: hits, the longest combo, the hits' reach summed.
    pub hits: u32,
    pub best: u32,
    reach_sum: f64,
    /// Reach History: the last `HISTORY` hits' reach, oldest first.
    pub recent: VecDeque<f64>,
}

pub const HISTORY: usize = 10;

/// An attack whose target is damaged this soon after counts as a hit.
pub const CONFIRM: u64 = 500;

impl Fight {
    pub fn attack(&mut self, target: i64, reach: f64, now: u64) {
        self.pending = Some((target, reach, now));
    }

    /// A damage event on `entity`; `me` is the player's own id.
    pub fn damaged(&mut self, entity: i64, me: Option<i64>, now: u64) {
        if Some(entity) == me {
            self.combo = 0;
            self.last_hit = None;
            return;
        }
        if let Some((target, reach, at)) = self.pending {
            if target == entity && now.saturating_sub(at) <= CONFIRM {
                self.pending = None;
                self.last_reach = Some((reach, now));
                self.combo += 1;
                self.last_hit = Some(now);
                self.target = Some((entity, now));
                self.hits += 1;
                self.best = self.best.max(self.combo);
                self.reach_sum += reach;
                self.recent.push_back(reach);
                if self.recent.len() > HISTORY {
                    self.recent.pop_front();
                }
            }
        }
    }

    /// The hits' mean reach, once there is a hit.
    pub fn average_reach(&self) -> Option<f64> {
        (self.hits > 0).then(|| self.reach_sum / f64::from(self.hits))
    }

    /// Session Stats start again.
    pub fn clear_stats(&mut self) {
        self.hits = 0;
        self.best = 0;
        self.reach_sum = 0.0;
        self.recent.clear();
    }

    /// The combo now, ended by `timeout` ms without a hit.
    pub fn combo(&mut self, now: u64, timeout: u64) -> u32 {
        if self.last_hit.is_some_and(|t| now.saturating_sub(t) > timeout) {
            self.combo = 0;
            self.last_hit = None;
        }
        self.combo
    }
}

/// When each key last went down or up, for Keystrokes' fade.
#[derive(Default)]
pub struct Fade(BTreeMap<&'static str, (bool, u64)>);

impl Fade {
    /// `down` now for `key`: how far (0 to 1) its box has turned to pressed.
    pub fn level(&mut self, key: &'static str, down: bool, now: u64, fade_ms: f64) -> f32 {
        // A key seen for the first time has been as it is for long.
        let e = self.0.entry(key).or_insert((down, u64::MAX));
        if e.0 != down {
            *e = (down, now);
        }
        let t = if fade_ms <= 0.0 || e.1 == u64::MAX { 1.0 } else { ((now.saturating_sub(e.1)) as f64 / fade_ms).min(1.0) as f32 };
        if down {
            t
        } else {
            1.0 - t
        }
    }
}

/// The view's turn, eased back to the centre, for Mousestrokes.
#[derive(Default)]
pub struct Mouse {
    last: Option<(f32, f32)>,
    pub dot: [f32; 2],
}

impl Mouse {
    /// The camera now (yaw, pitch); the dot follows how it turned.
    pub fn look(&mut self, yaw: f32, pitch: f32) {
        if let Some((y, p)) = self.last {
            let mut dy = yaw - y;
            if dy > 180.0 {
                dy -= 360.0;
            } else if dy < -180.0 {
                dy += 360.0;
            }
            self.dot[0] = (self.dot[0] + dy).clamp(-20.0, 20.0);
            self.dot[1] = (self.dot[1] + (pitch - p)).clamp(-20.0, 20.0);
        }
        self.dot = self.dot.map(|v| v * 0.85);
        self.last = Some((yaw, pitch));
    }
}

/// Blocks per second over the last half second of positions. A jump of
/// more than `TELEPORT` blocks between two frames (a teleport, a respawn)
/// starts the count again.
#[derive(Default)]
pub struct Speed(VecDeque<(u64, [f64; 3])>);

pub const TELEPORT: f64 = 10.0;

impl Speed {
    pub fn at(&mut self, now: u64, pos: [f64; 3], vertical: bool) -> f64 {
        let dist = |a: [f64; 3], b: [f64; 3]| {
            let dy = if vertical { b[1] - a[1] } else { 0.0 };
            ((b[0] - a[0]).powi(2) + dy * dy + (b[2] - a[2]).powi(2)).sqrt()
        };
        if self.0.back().is_some_and(|(_, last)| dist(*last, pos) > TELEPORT) {
            self.0.clear();
        }
        self.0.push_back((now, pos));
        while self.0.front().is_some_and(|(t, _)| now.saturating_sub(*t) > 500) {
            self.0.pop_front();
        }
        let (t0, p0) = self.0[0];
        let dt = now.saturating_sub(t0) as f64 / 1000.0;
        if dt <= 0.0 {
            0.0
        } else {
            dist(p0, pos) / dt
        }
    }
}

/// Where a yaw (Minecraft's: 0 faces +Z) looks: the direction's name and
/// the axis it runs along.
pub fn facing(yaw: f32) -> (&'static str, &'static str) {
    let y = yaw.rem_euclid(360.0);
    match ((y + 45.0) / 90.0) as u32 % 4 {
        0 => ("South", "+Z"),
        1 => ("West", "-X"),
        2 => ("North", "-Z"),
        _ => ("East", "+X"),
    }
}

/// The same place in the other dimension: (its name, x, z); none from the End.
pub fn other_dimension(dimension: &str, x: f64, z: f64) -> Option<(&'static str, f64, f64)> {
    if dimension.ends_with("the_nether") {
        Some(("Overworld", x * 8.0, z * 8.0))
    } else if dimension.ends_with("overworld") {
        Some(("Nether", x / 8.0, z / 8.0))
    } else {
        None
    }
}

// ---------------------------------------------------------------- drawing

/// A server address as shown: the port dropped unless asked for
/// (`play.example.net:25565` → `play.example.net`, `[::1]:25565` → `[::1]`).
pub fn host(address: &str, show_port: bool) -> String {
    if show_port {
        return address.to_owned();
    }
    if address.starts_with('[') {
        return address.split_inclusive(']').next().unwrap_or(address).to_owned();
    }
    match address.rsplit_once(':') {
        Some((h, port)) if !h.contains(':') && !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => h.to_owned(),
        _ => address.to_owned(),
    }
}

/// Seconds as `M:SS`, or `H:MM:SS` from an hour on.
pub fn clock(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// The last entity hit, for Target HUD.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Target {
    pub name: String,
    pub health: f32,
    pub max: f32,
    pub absorption: f32,
    /// Armour points (0 to 20).
    pub armor: f32,
    pub hand: Option<String>,
    pub distance: f64,
}

/// Session Stats as shown.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stats {
    pub hits: u32,
    pub best: u32,
    pub average: Option<f64>,
}

/// What the modules show this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    /// forward, left, back, right, jump, sneak, sprint, attack, use.
    pub keys: [bool; 9],
    pub cps: [u32; 2],
    pub mouse: [f32; 2],
    /// The last hit's reach, while shown.
    pub reach: Option<f64>,
    pub combo: u32,
    pub ping: Option<u32>,
    pub fps: u32,
    /// Armour worn, head first: (item id, durability left as a fraction,
    /// when known).
    pub armor: Vec<(String, Option<f32>)>,
    /// The item in the main hand, the same way.
    pub hand: Option<(String, Option<f32>)>,
    /// Active effects: (name, seconds left when known).
    pub effects: Vec<(String, Option<u32>)>,
    /// Toggle Sprint: (sprint toggled on, sprinting now).
    pub sprint: (bool, bool),
    pub target: Option<Target>,
    /// Blocks per second.
    pub speed: f64,
    /// Feet position, yaw and dimension id, once in a world.
    pub pos: Option<([f64; 3], f32, String)>,
    pub stats: Stats,
    /// The server's address; None in single player or the menus.
    pub server: Option<String>,
    /// Seconds since joining this server or world.
    pub session: Option<u64>,
    /// The last hits' reach, oldest first.
    pub recent: Vec<f64>,
}

/// An item id as a short name: `minecraft:diamond_chestplate` → `Diamond Chestplate`.
pub fn item_name(id: &str) -> String {
    id.rsplit(':')
        .next()
        .unwrap_or(id)
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or_else(String::new, |f| f.to_uppercase().collect::<String>() + c.as_str())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The colour (RGB) Reach Display writes a hit's distance in; None keeps the
/// module's text colour. Vanilla's server accepts attacks up to 3 blocks
/// (survival) from the eyes to the box, a little more with lag.
fn reach_color(reach: f64) -> Option<u32> {
    // Normal hits keep the module's own text colour; only reach the server
    // would reject without lag stands out: yellow up to 3.3 (lag slack),
    // red past that.
    if reach <= 3.0 {
        None
    } else if reach <= 3.3 {
        Some(0xFFFF55)
    } else {
        Some(0xFF5555)
    }
}

/// `text` after the module's `label`, when it has one.
fn labelled(o: &Options, text: String) -> String {
    let label = o.text("label");
    if label.is_empty() {
        text
    } else {
        format!("{label} {text}")
    }
}

fn wrap(o: &Options, text: String) -> String {
    if o.on("brackets") {
        format!("[{text}]")
    } else {
        text
    }
}

/// Lines of text in a box at `at`: the draws and the box's size.
fn lines(o: &Options, at: [f32; 2], rows: &[(String, Option<u32>)], measure: &dyn Fn(&str) -> f32) -> (Vec<Draw>, [f32; 2]) {
    let w = rows.iter().map(|(t, _)| measure(t)).fold(0.0, f32::max) + 8.0;
    let h = rows.len() as f32 * 10.0 + 4.0;
    let mut out = Vec::new();
    if o.on("background") {
        out.push(Draw::Fill([at[0], at[1], w, h], o.color("bg_color")));
    }
    let text = o.color("text_color");
    for (i, (t, rgb)) in rows.iter().enumerate() {
        out.push(Draw::Text(t.clone(), [at[0] + 4.0, at[1] + 3.0 + i as f32 * 10.0], rgb.unwrap_or(text), true));
    }
    (out, [w, h])
}

fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |s: u32| -> u32 {
        let (x, y) = (f32::from(((a >> s) & 0xFF) as u8), f32::from(((b >> s) & 0xFF) as u8));
        ((x + (y - x) * t).round() as u32) << s
    };
    ch(24) | ch(16) | ch(8) | ch(0)
}

/// A HUD module's draws at `at` and its size; `fade` gives each key's
/// pressed level. None for the world modules.
pub fn draw(o: &Options, snap: &Snapshot, at: [f32; 2], fade: &mut dyn FnMut(&'static str, bool) -> f32, measure: &dyn Fn(&str) -> f32) -> Option<(Vec<Draw>, [f32; 2])> {
    let row = |t: String| vec![(wrap(o, t), None)];
    let out = match o.module {
        Module::Keystrokes => keystrokes(o, snap, at, fade, measure),
        Module::Cps => {
            let [l, r] = snap.cps;
            let label = o.text("label");
            let t = match o.text("buttons").as_str() {
                "left" => format!("{l} {label}"),
                "right" => format!("{r} {label}"),
                _ => format!("{l} | {r} {label}"),
            };
            lines(o, at, &row(t), measure)
        }
        Module::Mousestrokes => {
            let s = o.number("size").clamp(16.0, 120.0) as f32;
            let k = o.number("sensitivity") as f32;
            let mut out = Vec::new();
            if o.on("background") {
                out.push(Draw::Fill([at[0], at[1], s, s], o.color("bg_color")));
            }
            let c = [at[0] + s / 2.0, at[1] + s / 2.0];
            let half = s / 2.0 - 3.0;
            let p = [c[0] + (snap.mouse[0] * k / 20.0 * half).clamp(-half, half), c[1] + (snap.mouse[1] * k / 20.0 * half).clamp(-half, half)];
            out.push(Draw::Fill([c[0] - 0.5, at[1] + 2.0, 1.0, s - 4.0], 0x30FF_FFFF));
            out.push(Draw::Fill([at[0] + 2.0, c[1] - 0.5, s - 4.0, 1.0], 0x30FF_FFFF));
            out.push(Draw::Fill([p[0] - 2.0, p[1] - 2.0, 4.0, 4.0], 0xFF00_0000 | o.color("dot")));
            (out, [s, s])
        }
        Module::Reach => {
            let d = o.number("decimals").clamp(0.0, 4.0) as usize;
            let t = snap.reach.map_or_else(|| "—".to_owned(), |r| format!("{r:.d$}"));
            let rgb = snap.reach.and_then(reach_color);
            lines(o, at, &[(wrap(o, format!("{t} {}", o.text("label"))), rgb)], measure)
        }
        Module::Combo => {
            if snap.combo == 0 && o.on("hide_zero") {
                return Some((Vec::new(), [measure("0 Combo") + 8.0, 14.0]));
            }
            let t = if snap.combo == 0 { format!("No {}", o.text("label")) } else { format!("{} {}", snap.combo, o.text("label")) };
            lines(o, at, &row(t), measure)
        }
        Module::ToggleSprint => {
            if !o.on("show_text") {
                return Some((Vec::new(), [0.0, 0.0]));
            }
            let t = match snap.sprint {
                (true, _) => "[Sprinting (Toggled)]",
                (false, true) => "[Sprinting (Vanilla)]",
                _ => "",
            };
            if t.is_empty() {
                return Some((Vec::new(), [measure("[Sprinting (Toggled)]") + 8.0, 14.0]));
            }
            lines(o, at, &[(t.to_owned(), None)], measure)
        }
        Module::ArmorStatus => {
            let hand = if o.on("hand") { snap.hand.as_ref() } else { None };
            let rows: Vec<(String, Option<u32>)> = snap
                .armor
                .iter()
                .chain(hand)
                .map(|(id, left)| {
                    let name = item_name(id);
                    match left {
                        Some(f) => {
                            let pct = (f * 100.0).round() as u32;
                            let rgb = if pct > 60 { 0x55FF55 } else if pct > 25 { 0xFFFF55 } else { 0xFF5555 };
                            (format!("{name} {pct}%"), Some(rgb))
                        }
                        None => (name, None),
                    }
                })
                .collect();
            if rows.is_empty() {
                return Some((Vec::new(), [measure("Diamond Chestplate 100%") + 8.0, 44.0]));
            }
            if o.on("vertical") {
                lines(o, at, &rows, measure)
            } else {
                let one = rows.iter().map(|r| r.0.clone()).collect::<Vec<_>>().join("  ");
                lines(o, at, &[(one, None)], measure)
            }
        }
        Module::PotionEffects => {
            let rows: Vec<(String, Option<u32>)> = snap
                .effects
                .iter()
                .map(|(name, left)| match left {
                    Some(s) => (format!("{name} {}:{:02}", s / 60, s % 60), None),
                    None => (name.clone(), None),
                })
                .collect();
            if rows.is_empty() {
                return Some((Vec::new(), [measure("Speed II 1:30") + 8.0, 14.0]));
            }
            if o.on("vertical") {
                lines(o, at, &rows, measure)
            } else {
                let one = rows.iter().map(|r| r.0.clone()).collect::<Vec<_>>().join("  ");
                lines(o, at, &[(one, None)], measure)
            }
        }
        Module::Ping => {
            let t = snap.ping.map_or_else(|| "?".to_owned(), |p| p.to_string());
            lines(o, at, &row(format!("{t} {}", o.text("label"))), measure)
        }
        Module::Fps => lines(o, at, &row(format!("{} {}", snap.fps, o.text("label"))), measure),
        Module::TargetHud => {
            let Some(t) = &snap.target else {
                return Some((Vec::new(), [measure("Player_Name_1234") + 8.0, 39.0]));
            };
            let frac = if t.max > 0.0 { (t.health / t.max).clamp(0.0, 1.0) } else { 0.0 };
            let hp_rgb = if frac > 0.6 { 0x55FF55 } else if frac > 0.3 { 0xFFFF55 } else { 0xFF5555 };
            let mut hp = format!("HP {:.1} / {:.0}", t.health, t.max);
            if t.absorption > 0.0 {
                hp += &format!(" +{:.0}", t.absorption);
            }
            let mut rows = vec![(t.name.clone(), None), (hp, Some(hp_rgb))];
            let mut extra = Vec::new();
            if o.on("show_armor") && t.armor > 0.0 {
                extra.push(format!("Armor {:.0}", t.armor));
            }
            if o.on("show_distance") {
                extra.push(format!("{:.1} m", t.distance));
            }
            if !extra.is_empty() {
                rows.push((extra.join("  "), None));
            }
            if let (true, Some(h)) = (o.on("show_hand"), &t.hand) {
                rows.push((item_name(h), Some(0xAAAAAA)));
            }
            let (mut out, mut size) = lines(o, at, &rows, measure);
            if o.on("health_bar") {
                // A strip under the text: grey track, the health over it.
                let y = at[1] + size[1];
                if o.on("background") {
                    out.push(Draw::Fill([at[0], y, size[0], 5.0], o.color("bg_color")));
                }
                let inner = size[0] - 8.0;
                out.push(Draw::Fill([at[0] + 4.0, y, inner, 3.0], 0xFF40_4040));
                out.push(Draw::Fill([at[0] + 4.0, y, inner * frac, 3.0], 0xFF00_0000 | hp_rgb));
                size[1] += 5.0;
            }
            (out, size)
        }
        Module::Speed => {
            let d = o.number("decimals").clamp(0.0, 4.0) as usize;
            lines(o, at, &row(format!("{:.d$} {}", snap.speed, o.text("label"))), measure)
        }
        Module::Coordinates => {
            let Some((p, yaw, dim)) = &snap.pos else {
                return Some((Vec::new(), [measure("XYZ: -12345 64 -12345") + 8.0, 34.0]));
            };
            let d = o.number("decimals").clamp(0.0, 3.0) as usize;
            let f = |v: f64| format!("{v:.d$}");
            let mut rows = vec![(format!("XYZ: {} {} {}", f(p[0]), f(p[1]), f(p[2])), None)];
            if o.on("direction") {
                let (name, axis) = facing(*yaw);
                rows.push((format!("Facing: {name} ({axis})"), None));
            }
            if o.on("other_dimension") {
                if let Some((name, x, z)) = other_dimension(dim, p[0], p[2]) {
                    rows.push((format!("{name}: {} {}", f(x), f(z)), Some(0xAAAAAA)));
                }
            }
            lines(o, at, &rows, measure)
        }
        Module::Stats => {
            let s = &snap.stats;
            let mut rows = Vec::new();
            if o.on("show_hits") {
                rows.push((format!("Hits: {}", s.hits), None));
            }
            if o.on("show_best") {
                rows.push((format!("Best Combo: {}", s.best), None));
            }
            if o.on("show_average") {
                let avg = s.average.map_or_else(|| "—".to_owned(), |r| format!("{r:.2}"));
                rows.push((format!("Avg Reach: {avg}"), None));
            }
            if rows.is_empty() {
                return Some((Vec::new(), [0.0, 0.0]));
            }
            lines(o, at, &rows, measure)
        }
        Module::ServerAddress => {
            let shown = snap.server.as_deref().map_or_else(|| "Singleplayer".to_owned(), |a| host(a, o.on("show_port")));
            lines(o, at, &row(labelled(o, shown)), measure)
        }
        Module::SessionTime => {
            let Some(secs) = snap.session else {
                return Some((Vec::new(), [measure("0:00:00") + 8.0, 14.0]));
            };
            lines(o, at, &row(labelled(o, clock(secs))), measure)
        }
        Module::ReachHistory => {
            let d = o.number("decimals").clamp(0.0, 4.0) as usize;
            let n = (o.number("count").clamp(1.0, HISTORY as f64)) as usize;
            let last: Vec<(String, Option<u32>)> =
                snap.recent.iter().rev().take(n).map(|r| (format!("{r:.d$}"), reach_color(*r))).collect();
            if last.is_empty() {
                return Some((Vec::new(), [measure("3.00 3.00 3.00") + 8.0, 14.0]));
            }
            if o.on("vertical") {
                lines(o, at, &last, measure)
            } else {
                // One line, newest first; the colours stay per hit, so each
                // value is drawn on its own after the box.
                let texts: Vec<&str> = last.iter().map(|r| r.0.as_str()).collect();
                let (mut out, size) = lines(o, at, &[(texts.join("  "), None)], measure);
                out.retain(|d| !matches!(d, Draw::Text(..)));
                let mut x = at[0] + 4.0;
                let text = o.color("text_color");
                for (t, rgb) in &last {
                    out.push(Draw::Text(t.clone(), [x, at[1] + 3.0], rgb.unwrap_or(text), true));
                    x += measure(t) + measure("  ");
                }
                (out, size)
            }
        }
        Module::Hitbox | Module::BlockOverlay => return None,
    };
    Some(out)
}

fn keystrokes(o: &Options, snap: &Snapshot, at: [f32; 2], fade: &mut dyn FnMut(&'static str, bool) -> f32, measure: &dyn Fn(&str) -> f32) -> (Vec<Draw>, [f32; 2]) {
    let b = o.number("box").clamp(10.0, 40.0) as f32;
    let gap = 2.0;
    let bg = o.color("bg_color");
    let pressed = o.color("pressed_bg");
    let text = o.color("text_color");
    let pressed_text = o.color("pressed_text");
    let arrows = o.on("arrows");
    let mut out = Vec::new();
    let mut key = |out: &mut Vec<Draw>, name: &'static str, label: String, down: bool, rect: [f32; 4]| {
        let t = fade(name, down);
        out.push(Draw::Fill(rect, mix(bg, pressed, t)));
        let rgb = mix(0xFF00_0000 | text, 0xFF00_0000 | pressed_text, t) & 0xFF_FFFF;
        let lines: Vec<&str> = label.lines().collect();
        for (i, l) in lines.iter().enumerate() {
            let w = measure(l);
            let y = rect[1] + (rect[3] - lines.len() as f32 * 9.0) / 2.0 + i as f32 * 9.0 + 1.0;
            out.push(Draw::Text((*l).to_owned(), [rect[0] + (rect[2] - w) / 2.0, y], rgb, t < 0.5));
        }
    };
    let full = 3.0 * b + 2.0 * gap;
    let mut y = at[1];
    if o.on("show_movement") {
        let [w, a, s, d] = if arrows { ["▲", "◀", "▼", "▶"] } else { ["W", "A", "S", "D"] };
        key(&mut out, "forward", w.into(), snap.keys[0], [at[0] + b + gap, y, b, b]);
        y += b + gap;
        key(&mut out, "left", a.into(), snap.keys[1], [at[0], y, b, b]);
        key(&mut out, "back", s.into(), snap.keys[2], [at[0] + b + gap, y, b, b]);
        key(&mut out, "right", d.into(), snap.keys[3], [at[0] + 2.0 * (b + gap), y, b, b]);
        y += b + gap;
    }
    if o.on("show_mouse") {
        let half = (full - gap) / 2.0;
        let cps = o.on("show_cps");
        let label = |n: &str, c: u32| if cps { format!("{n}\n{c} CPS") } else { n.to_owned() };
        key(&mut out, "attack", label("LMB", snap.cps[0]), snap.keys[7], [at[0], y, half, b]);
        key(&mut out, "use", label("RMB", snap.cps[1]), snap.keys[8], [at[0] + half + gap, y, half, b]);
        y += b + gap;
    }
    if o.on("show_space") {
        let h = (b / 2.0).max(8.0);
        key(&mut out, "jump", "▬▬▬".into(), snap.keys[4], [at[0], y, full, h]);
        y += h + gap;
    }
    (out, [full, (y - at[1] - gap).max(0.0)])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure(t: &str) -> f32 {
        t.chars().count() as f32 * 6.0
    }

    #[test]
    fn clicks_count_for_one_second() {
        let mut c = Clicks::default();
        for t in [0, 100, 200, 950] {
            c.press(t);
        }
        assert_eq!(c.per_second(999), 4);
        assert_eq!(c.per_second(1100), 2);
        assert_eq!(c.per_second(2000), 0);
    }

    #[test]
    fn reach_is_to_the_nearest_point_of_the_box() {
        let r = reach([0.0, 1.62, 0.0], [2.7, 0.0, -0.3], [3.3, 1.8, 0.3]);
        assert!((r - 2.7).abs() < 1e-9);
        let inside = reach([0.0, 1.0, 0.0], [-0.3, 0.0, -0.3], [0.3, 1.8, 0.3]);
        assert_eq!(inside, 0.0);
    }

    #[test]
    fn a_combo_needs_confirmed_hits_and_ends() {
        let mut f = Fight::default();
        f.attack(7, 2.9, 0);
        f.damaged(7, Some(1), 100);
        f.attack(7, 3.1, 400);
        f.damaged(7, Some(1), 450);
        assert_eq!(f.combo(500, 2000), 2);
        assert_eq!(f.last_reach.map(|r| r.0), Some(3.1));
        // A damage event long after the attack is someone else's hit.
        f.attack(7, 3.0, 1000);
        f.damaged(7, Some(1), 1700);
        assert_eq!(f.combo(1700, 2000), 2);
        // Being hit ends it.
        f.damaged(1, Some(1), 1800);
        assert_eq!(f.combo(1800, 2000), 0);
        f.attack(9, 2.0, 3000);
        f.damaged(9, Some(1), 3100);
        assert_eq!(f.combo(6000, 2000), 0, "timed out");
    }

    #[test]
    fn options_fall_back_to_defaults_and_check_values() {
        let stored = |k: &str| (k == "cps.buttons").then(|| "left".to_owned());
        let o = Options { module: Module::Cps, get: &stored };
        assert_eq!(o.text("buttons"), "left");
        assert!(o.on("background"));
        assert_eq!(o.color("bg_color"), 0x6F00_0000);
        assert_eq!(checked(Module::Cps, "buttons", "up"), Err("left, right ya da both olmalı.".into()));
        assert_eq!(checked(Module::Keystrokes, "box", "22"), Ok("22".into()));
        assert!(checked(Module::Keystrokes, "pressed_bg", "fff").is_err());
        assert_eq!(checked(Module::Fps, "enabled", "açık"), Ok("true".into()));
        assert!(checked(Module::Fps, "nope", "1").is_err());
    }

    #[test]
    fn keystrokes_lay_out_the_keys() {
        let none = |_: &str| None;
        let o = Options { module: Module::Keystrokes, get: &none };
        let snap = Snapshot { keys: [true, false, false, false, false, false, false, false, false], cps: [7, 0], ..Snapshot::default() };
        let mut fade = |_: &'static str, down: bool| if down { 1.0 } else { 0.0 };
        let (draws, size) = draw(&o, &snap, [0.0, 0.0], &mut fade, &measure).unwrap();
        assert_eq!(size[0], 3.0 * 18.0 + 4.0);
        assert!(draws.contains(&Draw::Fill([20.0, 0.0, 18.0, 18.0], 0xB4FF_FFFF)), "W pressed");
        assert!(draws.iter().any(|d| matches!(d, Draw::Text(t, _, _, _) if t == "7 CPS")));
    }

    #[test]
    fn cps_and_fps_read_their_counts() {
        let none = |_: &str| None;
        let snap = Snapshot { cps: [9, 3], fps: 240, ..Snapshot::default() };
        let mut fade = |_: &'static str, _: bool| 0.0;
        let mut text = |m: Module| -> String {
            let o = Options { module: m, get: &none };
            let (d, _) = draw(&o, &snap, [0.0, 0.0], &mut fade, &measure).unwrap();
            d.into_iter().find_map(|d| if let Draw::Text(t, ..) = d { Some(t) } else { None }).unwrap()
        };
        assert_eq!(text(Module::Cps), "9 | 3 CPS");
        assert_eq!(text(Module::Fps), "240 FPS");
    }

    #[test]
    fn fades_go_both_ways() {
        let mut f = Fade::default();
        assert_eq!(f.level("w", false, 0, 100.0), 0.0, "never pressed");
        assert_eq!(f.level("w", true, 1000, 100.0), 0.0);
        assert!((f.level("w", true, 1050, 100.0) - 0.5).abs() < 1e-6);
        assert_eq!(f.level("w", true, 1200, 100.0), 1.0);
        assert_eq!(f.level("w", false, 1300, 100.0), 1.0);
        assert_eq!(f.level("w", false, 1400, 100.0), 0.0);
    }

    #[test]
    fn hits_feed_the_session_stats_and_target() {
        let mut f = Fight::default();
        assert_eq!(f.average_reach(), None);
        for (i, r) in [2.0, 3.0, 4.0].into_iter().enumerate() {
            let t = i as u64 * 300;
            f.attack(5, r, t);
            f.damaged(5, Some(1), t + 50);
        }
        assert_eq!((f.hits, f.best), (3, 3));
        assert_eq!(f.average_reach(), Some(3.0));
        assert_eq!(f.target, Some((5, 650)));
        f.damaged(1, Some(1), 700);
        assert_eq!(f.best, 3, "the best combo outlives the combo");
        f.clear_stats();
        assert_eq!((f.hits, f.best, f.average_reach()), (0, 0, None));
    }

    #[test]
    fn speed_counts_blocks_per_second_and_skips_teleports() {
        let mut s = Speed::default();
        assert_eq!(s.at(0, [0.0, 64.0, 0.0], false), 0.0);
        assert!((s.at(250, [1.0, 64.0, 0.0], false) - 4.0).abs() < 1e-9);
        assert!((s.at(500, [2.0, 70.0, 0.0], false) - 4.0).abs() < 1e-9, "height ignored");
        assert_eq!(s.at(550, [500.0, 64.0, 0.0], false), 0.0, "teleport");
    }

    #[test]
    fn facing_and_the_other_dimension() {
        assert_eq!(facing(0.0).0, "South");
        assert_eq!(facing(90.0).0, "West");
        assert_eq!(facing(-90.0).0, "East");
        assert_eq!(facing(179.0).0, "North");
        assert_eq!(facing(-135.0 - 0.1).0, "North");
        assert_eq!(other_dimension("minecraft:overworld", 800.0, -80.0), Some(("Nether", 100.0, -10.0)));
        assert_eq!(other_dimension("minecraft:the_nether", 1.0, 2.0), Some(("Overworld", 8.0, 16.0)));
        assert_eq!(other_dimension("minecraft:the_end", 1.0, 2.0), None);
    }

    #[test]
    fn target_hud_shows_the_last_hit() {
        let none = |_: &str| None;
        let o = Options { module: Module::TargetHud, get: &none };
        let mut fade = |_: &'static str, _: bool| 0.0;
        let snap = Snapshot {
            target: Some(Target { name: "Rakip".into(), health: 5.0, max: 20.0, armor: 12.0, distance: 2.84, hand: Some("minecraft:iron_sword".into()), ..Target::default() }),
            ..Snapshot::default()
        };
        let (draws, size) = draw(&o, &snap, [0.0, 0.0], &mut fade, &measure).unwrap();
        let texts: Vec<String> = draws.iter().filter_map(|d| if let Draw::Text(t, ..) = d { Some(t.clone()) } else { None }).collect();
        assert_eq!(texts, ["Rakip", "HP 5.0 / 20", "Armor 12  2.8 m", "Iron Sword"]);
        assert!(draws.iter().any(|d| matches!(d, Draw::Text(t, _, 0xFF5555, _) if t.starts_with("HP"))), "a quarter is red");
        assert_eq!(size[1], 4.0 * 10.0 + 4.0 + 5.0);
    }

    #[test]
    fn addresses_clocks_and_reach_history() {
        assert_eq!(host("play.example.net:25565", false), "play.example.net");
        assert_eq!(host("play.example.net:25565", true), "play.example.net:25565");
        assert_eq!(host("[::1]:25565", false), "[::1]");
        assert_eq!(host("2001:db8::1", false), "2001:db8::1", "a bare IPv6 keeps its colons");
        assert_eq!(clock(59), "0:59");
        assert_eq!(clock(3 * 3600 + 5 * 60 + 7), "3:05:07");
        let mut f = Fight::default();
        for i in 0..12 {
            f.attack(3, i as f64, i * 1000);
            f.damaged(3, Some(1), i * 1000 + 10);
        }
        assert_eq!(f.recent.len(), HISTORY);
        assert_eq!(f.recent.front(), Some(&2.0));
        let none = |_: &str| None;
        let o = Options { module: Module::ReachHistory, get: &none };
        let snap = Snapshot { recent: vec![2.5, 3.1, 3.6], ..Snapshot::default() };
        let mut fade = |_: &'static str, _: bool| 0.0;
        let (draws, _) = draw(&o, &snap, [0.0, 0.0], &mut fade, &measure).unwrap();
        let texts: Vec<(String, u32)> = draws.into_iter().filter_map(|d| if let Draw::Text(t, _, c, _) = d { Some((t, c)) } else { None }).collect();
        assert_eq!(texts, [("3.60".into(), 0xFF5555), ("3.10".into(), 0xFFFF55), ("2.50".into(), 0xFFFFFF)], "newest first, coloured");
    }

    #[test]
    fn names_read_well() {
        assert_eq!(item_name("minecraft:diamond_chestplate"), "Diamond Chestplate");
        assert_eq!(mix(0x0000_0000, 0xFFFF_FFFF, 0.5), 0x8080_8080);
    }
}
