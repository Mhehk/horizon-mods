// Remove Reloading Screen — mhv2 (mihehk): a Horizon port of dima_dencep's
// Remove Reloading Screen (RRLS, https://github.com/dima-dencep/rrls). Like the
// original it is licensed under the Open Software License 3.0 (LICENSE beside
// this mod).
//
// Every resource reload (the pack screen, F3+T, a server's packs) stages on its
// worker out of sight and goes back to the game once done, oldest first, while
// a small version of the overlay (its progress bar or a line of text) is drawn
// over the game and fades out (`LoadingOverlay.rrls$miniRender`). The settings
// are RRLS's own: `config/rrls-client.toml`, the same sections, keys, defaults
// and captions, set on Options > Mods > this mod, where a section shows its
// preview overlay as RRLS's section screens do.
use core::ops::ControlFlow;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, SystemTime};

use horizon_client::hud::{HudAtlas, HudBatch, rgba};
use horizon_client::text::Text;

pub(crate) const ID: &str = "noreloadscreen";

// ---------------------------------------------------------------------------------------------
// Configuration (ForgeLikeConfigImpl / FallbackConfigImpl, en_us.json)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Kind {
    Bool,
    Enum(&'static [(&'static str, &'static str)]),
    Text,
    /// A number; the page's button steps through these values.
    Number(&'static [f64]),
}

struct Entry {
    section: usize,
    key: &'static str,
    caption: &'static str,
    kind: Kind,
    default: &'static str,
}

const SECTIONS: [(&str, &str); 5] = [
    ("global", "Global"),
    ("splash", "Splash overlay"),
    ("interpolation", "Interpolation"),
    ("other", "Other"),
    ("platform", "Platform-specific"),
];

/// `HideType`.
const HIDE: &[(&str, &str)] = &[
    ("ALL", "All overlays"),
    ("LOADING", "Loading only"),
    ("RELOADING", "Reloading only"),
    ("NONE", "None"),
];
/// `Type`.
const TYPE: &[(&str, &str)] = &[
    ("PROGRESS", "Progress Bar"),
    ("TEXT", "Custom text"),
    ("TEXT_WITH_BACKGROUND", "Custom text with background"),
    ("NONE", "Nothing"),
];
/// `DoubleLoad`.
const DOUBLE_LOAD: &[(&str, &str)] = &[
    ("FORCE_LOAD", "Force load"),
    ("LOAD", "Load"),
    ("NONE", "None"),
];
/// `Ease`: not translated, so the screen shows the names.
const EASE: &[(&str, &str)] = &[
    ("LINEAR", "LINEAR"),
    ("CONSTANT", "CONSTANT"),
    ("INSINE", "INSINE"),
    ("OUTSINE", "OUTSINE"),
    ("INOUTSINE", "INOUTSINE"),
    ("INCUBIC", "INCUBIC"),
    ("OUTCUBIC", "OUTCUBIC"),
    ("INOUTCUBIC", "INOUTCUBIC"),
    ("INQUAD", "INQUAD"),
    ("OUTQUAD", "OUTQUAD"),
    ("INOUTQUAD", "INOUTQUAD"),
    ("INQUART", "INQUART"),
    ("OUTQUART", "OUTQUART"),
    ("INOUTQUART", "INOUTQUART"),
    ("INQUINT", "INQUINT"),
    ("OUTQUINT", "OUTQUINT"),
    ("INOUTQUINT", "INOUTQUINT"),
    ("INEXPO", "INEXPO"),
    ("OUTEXPO", "OUTEXPO"),
    ("INOUTEXPO", "INOUTEXPO"),
    ("INCIRC", "INCIRC"),
    ("OUTCIRC", "OUTCIRC"),
    ("INOUTCIRC", "INOUTCIRC"),
    ("INBACK", "INBACK"),
    ("OUTBACK", "OUTBACK"),
    ("INOUTBACK", "INOUTBACK"),
    ("INELASTIC", "INELASTIC"),
    ("OUTELASTIC", "OUTELASTIC"),
    ("INOUTELASTIC", "INOUTELASTIC"),
    ("INBOUNCE", "INBOUNCE"),
    ("OUTBOUNCE", "OUTBOUNCE"),
    ("INOUTBOUNCE", "INOUTBOUNCE"),
    ("CATMULLROM", "CATMULLROM"),
    ("STEP", "STEP"),
];

const ENTRIES: &[Entry] = &[
    Entry { section: 0, key: "hideOverlays", caption: "Overlays that will be hidden", kind: Kind::Enum(HIDE), default: "ALL" },
    Entry { section: 0, key: "blockOverlay", caption: "Complete blocking of overlay usage", kind: Kind::Bool, default: "false" },
    Entry { section: 0, key: "miniRender", caption: "Render a mini (aka hidden) version of the overlay", kind: Kind::Bool, default: "true" },
    Entry { section: 0, key: "enableScissor", caption: "Trim the rendering of the default overlay", kind: Kind::Bool, default: "false" },
    Entry { section: 1, key: "type", caption: "What will be displayed on reloading", kind: Kind::Enum(TYPE), default: "PROGRESS" },
    Entry { section: 1, key: "rgbProgress", caption: "Colorful", kind: Kind::Bool, default: "false" },
    Entry { section: 1, key: "reloadText", caption: "The text that will be displayed if TEXT is selected above", kind: Kind::Text, default: "Edit in config!" },
    Entry { section: 1, key: "animationSpeed", caption: "Splash animation speed", kind: Kind::Number(&[250.0, 500.0, 750.0, 1000.0, 1500.0, 2000.0, 3000.0, 5000.0]), default: "1000.0" },
    Entry { section: 2, key: "interpolateProgress", caption: "Interpolate progress", kind: Kind::Bool, default: "false" },
    Entry { section: 2, key: "interpolateAtEnd", caption: "Interpolate at the end", kind: Kind::Bool, default: "true" },
    Entry { section: 2, key: "ease", caption: "Easing", kind: Kind::Enum(EASE), default: "INOUTQUINT" },
    Entry { section: 2, key: "easingArg", caption: "Easing argument", kind: Kind::Number(&[f64::NAN, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0]), default: "NaN" },
    Entry { section: 3, key: "resetResources", caption: "Reset resources when failed load", kind: Kind::Bool, default: "true" },
    Entry { section: 3, key: "reInitScreen", caption: "Reinitializing the current screen after a reload", kind: Kind::Bool, default: "true" },
    Entry { section: 3, key: "earlyPackStatusSend", caption: "Early sending of status about server resourcepack", kind: Kind::Bool, default: "false" },
    Entry { section: 3, key: "doubleLoad", caption: "Reload resources again if an error occurs", kind: Kind::Enum(DOUBLE_LOAD), default: "FORCE_LOAD" },
    Entry { section: 4, key: "skipForgeOverlay", caption: "Replace the (Neo)Forge loading overlay with vanilla", kind: Kind::Bool, default: "false" },
];

struct Values {
    map: BTreeMap<&'static str, String>,
    stamp: Option<SystemTime>,
    checked: Instant,
}

thread_local! {
    static VALUES: RefCell<Option<Values>> = const { RefCell::new(None) };
}

fn path() -> Option<PathBuf> {
    crate::game::Game::native_config_dir()
        .ok()
        .map(|d| d.join("rrls-client.toml"))
}

fn stamp() -> Option<SystemTime> {
    std::fs::metadata(path()?).and_then(|m| m.modified()).ok()
}

fn fmt_num(v: f64) -> String {
    if v.is_nan() { "NaN".into() } else { format!("{v:?}") }
}

fn parse_num(s: &str) -> Option<f64> {
    match s.trim() {
        "nan" | "NaN" | "+nan" | "-nan" => Some(f64::NAN),
        s => s.parse::<f64>().ok(),
    }
}

/// A value from the file as the entry keeps it, or None when it is not one.
fn normalize(e: &Entry, raw: &str) -> Option<String> {
    let raw = raw.trim();
    match e.kind {
        Kind::Bool => match raw {
            "true" | "false" => Some(raw.into()),
            _ => None,
        },
        Kind::Enum(names) => {
            let v = raw.trim_matches('"');
            names.iter().any(|(n, _)| *n == v).then(|| v.to_owned())
        }
        Kind::Text => {
            let v = raw.strip_prefix('"')?.strip_suffix('"')?;
            Some(v.replace("\\\"", "\"").replace("\\\\", "\\"))
        }
        Kind::Number(_) => parse_num(raw.trim_matches('"')).map(fmt_num),
    }
}

fn defaults() -> BTreeMap<&'static str, String> {
    ENTRIES.iter().map(|e| (e.key, e.default.to_owned())).collect()
}

fn read_file() -> BTreeMap<&'static str, String> {
    let mut map = defaults();
    let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) else {
        return map;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if let Some(e) = ENTRIES.iter().find(|e| e.key == key)
            && let Some(v) = normalize(e, value)
        {
            map.insert(e.key, v);
        }
    }
    map
}

/// The file as NeoForge's config writes it (night-config TOML).
fn write_file(map: &BTreeMap<&'static str, String>) {
    let Some(path) = path() else {
        return;
    };
    let mut out = String::new();
    for (s, (section, _)) in SECTIONS.iter().enumerate() {
        if s > 0 {
            out.push('\n');
        }
        out.push_str(&format!("[{section}]\n"));
        for e in ENTRIES.iter().filter(|e| e.section == s) {
            let v = map.get(e.key).cloned().unwrap_or_else(|| e.default.into());
            let value = match e.kind {
                Kind::Bool => v,
                Kind::Enum(names) => {
                    let all: Vec<&str> = names.iter().map(|(n, _)| *n).collect();
                    out.push_str(&format!("\t#Allowed Values: {}\n", all.join(", ")));
                    format!("\"{v}\"")
                }
                Kind::Text => format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\"")),
                Kind::Number(_) => {
                    if v == "NaN" { "nan".into() } else { v }
                }
            };
            out.push_str(&format!("\t{} = {}\n", e.key, value));
        }
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, out);
}

fn with_values<R>(f: impl FnOnce(&mut Values) -> R) -> R {
    VALUES.with(|cell| {
        let mut cell = cell.borrow_mut();
        let v = cell.get_or_insert_with(|| {
            let missing = path().is_some_and(|p| !p.exists());
            let map = read_file();
            if missing {
                write_file(&map);
            }
            Values { map, stamp: stamp(), checked: Instant::now() }
        });
        f(v)
    })
}

fn get(key: &str) -> String {
    with_values(|v| v.map.get(key).cloned()).unwrap_or_default()
}

fn set(key: &'static str, value: String) {
    with_values(|v| {
        v.map.insert(key, value);
        write_file(&v.map);
        v.stamp = stamp();
    });
}

/// The file edited by hand: read again once its time changes (checked each second).
fn watch(now: Instant) {
    with_values(|v| {
        if now.duration_since(v.checked) < Duration::from_secs(1) {
            return;
        }
        v.checked = now;
        let s = stamp();
        if s != v.stamp {
            v.stamp = s;
            v.map = read_file();
        }
    });
}

fn index(key: &str, names: &[(&str, &str)]) -> usize {
    let v = get(key);
    names.iter().position(|(n, _)| *n == v).unwrap_or(0)
}

/// The values the overlay reads (`RrlsConfig.INSTANCE`).
struct Cfg {
    hide: usize,
    mini: bool,
    kind: usize,
    rgb: bool,
    text: String,
    speed: f32,
    interpolate_progress: bool,
    interpolate_at_end: bool,
    ease: usize,
    arg: Option<f32>,
    reset_resources: bool,
    early: bool,
    double_load: usize,
}

fn cfg() -> Cfg {
    let num = |k: &str| parse_num(&get(k)).unwrap_or(f64::NAN) as f32;
    let arg = num("easingArg");
    Cfg {
        hide: index("hideOverlays", HIDE),
        mini: get("miniRender") == "true",
        kind: index("type", TYPE),
        rgb: get("rgbProgress") == "true",
        text: get("reloadText"),
        speed: num("animationSpeed"),
        interpolate_progress: get("interpolateProgress") == "true",
        interpolate_at_end: get("interpolateAtEnd") == "true",
        ease: index("ease", EASE),
        arg: (!arg.is_nan()).then_some(arg),
        reset_resources: get("resetResources") == "true",
        early: get("earlyPackStatusSend") == "true",
        double_load: index("doubleLoad", DOUBLE_LOAD),
    }
}

impl Cfg {
    /// `HideType.canHide(reloading)`.
    fn can_hide(&self, reloading: bool) -> bool {
        match self.hide {
            0 => true,
            1 => !reloading,
            2 => reloading,
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Easings (Ease, Easing)
// ---------------------------------------------------------------------------------------------

fn base(name: &str, n: f32, arg: Option<f32>) -> Option<f32> {
    use std::f64::consts::PI;
    Some(match name {
        "LINEAR" => n,
        "CONSTANT" => 0.0,
        "SINE" => 1.0 - (f64::from(n) * PI / 2.0).cos() as f32,
        "CUBIC" => n * n * n,
        "QUAD" => n * n,
        "QUART" => f64::from(n).powf(4.0) as f32,
        "QUINT" => f64::from(n).powf(5.0) as f32,
        "EXPO" => 2f64.powf(f64::from(10.0 * (n - 1.0))) as f32,
        "CIRC" => 1.0 - f64::from(1.0 - n * n).sqrt() as f32,
        "BACK" => {
            let n2 = arg.map_or(1.70158, |a| a * 1.70158);
            n * n * ((n2 + 1.0) * n - n2)
        }
        "ELASTIC" => {
            let n2 = arg.unwrap_or(1.0);
            (1.0 - (f64::from(n) * PI / 2.0).cos().powi(3) * (f64::from(n * n2) * PI).cos()) as f32
        }
        "BOUNCE" => {
            let n2 = f64::from(arg.unwrap_or(0.5));
            let x = f64::from(n);
            let one = 121.0f32 / 16.0 * n * n;
            let two = (121.0 / 4.0 * n2 * (x - 6.0 / 11.0).powi(2) + 1.0 - n2) as f32;
            let three = (121.0 * n2 * n2 * (x - 9.0 / 11.0).powi(2) + 1.0 - n2 * n2) as f32;
            let four = (484.0 * n2 * n2 * n2 * (x - 10.5 / 11.0).powi(2) + 1.0 - n2 * n2 * n2) as f32;
            one.min(two).min(three.min(four))
        }
        "CATMULLROM" => {
            0.5 * (2.0 * (n + 1.0)
                + ((n + 2.0) - n)
                + (2.0 * n - 5.0 * (n + 1.0) + 4.0 * (n + 2.0) - (n + 3.0))
                + (3.0 * (n + 1.0) - n - 3.0 * (n + 2.0) + (n + 3.0)))
        }
        "STEP" => {
            let n2 = arg.unwrap_or(2.0);
            if n2 < 2.0 {
                // "Steps must be >= 2": the overlay fails to draw.
                return None;
            }
            let steps = n2 as i32;
            if n < 0.0 {
                return Some(0.0);
            }
            let step = 1.0 / steps as f32;
            let last = (steps - 1) as f32 * step;
            if n > last {
                return Some(last);
            }
            let (mut left, mut right) = (0, steps - 1);
            while right - left != 1 {
                let test = left + (right - left) / 2;
                if n >= test as f32 * step {
                    left = test;
                } else {
                    right = test;
                }
            }
            left as f32 * step
        }
        _ => n,
    })
}

/// `Ease.invoke(t, arg)`.
fn invoke(ease: usize, t: f32, arg: Option<f32>) -> Option<f32> {
    let name = EASE.get(ease).map_or("LINEAR", |(n, _)| *n);
    let (mode, f) = match name {
        "LINEAR" | "CONSTANT" | "STEP" => (0, name),
        "CATMULLROM" => (2, name),
        _ if name.starts_with("INOUT") => (2, &name[5..]),
        _ if name.starts_with("OUT") => (1, &name[3..]),
        _ => (0, &name[2..]),
    };
    match mode {
        0 => base(f, t, arg),
        1 => Some(1.0 - base(f, 1.0 - t, arg)?),
        _ if t < 0.5 => Some(base(f, t * 2.0, arg)? / 2.0),
        _ => Some(1.0 - base(f, (1.0 - t) * 2.0, arg)? / 2.0),
    }
}

// ---------------------------------------------------------------------------------------------
// The overlay (LoadingOverlay with RRLS's state)
// ---------------------------------------------------------------------------------------------

struct Overlay {
    /// The reload out of sight, until it goes back to the game.
    reload: Option<super::PackReload>,
    /// Given back and applied (`onFinish` ran, `fadeOutStart` set).
    applied: bool,
    /// The section screen's own overlay (`ConfigurationSectionScreenWithOverlay`).
    preview: bool,
    started: Instant,
    /// `currentProgress`.
    progress: f32,
    fade_out: Option<Instant>,
    /// `rrls$atEndStart`.
    at_end: Option<Instant>,
    /// `rrls$isFinished`.
    finished: bool,
}

impl Overlay {
    fn new(reload: Option<super::PackReload>, preview: bool, now: Instant) -> Self {
        Self {
            reload,
            applied: false,
            preview,
            started: now,
            progress: 0.0,
            fade_out: None,
            at_end: None,
            finished: false,
        }
    }

    /// `ReloadInstance.getActualProgress`: the preview's own adds 0.1 a tick.
    fn actual(&self, now: Instant) -> f32 {
        if self.preview {
            (now.duration_since(self.started).as_secs_f32() * 2.0).clamp(0.0, 1.0)
        } else if let Some(r) = &self.reload {
            (r.progress.fraction() as f32).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    fn update(&mut self, now: Instant) {
        let s = self.actual(now);
        self.progress = (self.progress * 0.95 + s * 0.050000012).clamp(0.0, 1.0);
        if self.at_end.is_none() && self.progress >= 0.99999 {
            self.at_end = Some(now);
        }
        // The preview's `isDone` (> 1.5): `fadeIn` is false, so it finishes at once.
        if self.preview
            && self.fade_out.is_none()
            && now.duration_since(self.started).as_secs_f32() * 2.0 > 1.5
        {
            self.fade_out = Some(now);
        }
    }

    /// The fade of the mini overlay, 1 down to 0 (None: the easing failed).
    fn ease(&self, c: &Cfg, now: Instant) -> Option<f32> {
        let fade_time = if c.interpolate_at_end { self.at_end } else { self.fade_out };
        let mut ease = 1.0;
        if c.interpolate_progress {
            ease -= invoke(c.ease, self.progress, c.arg)?;
        } else if let Some(t) = fade_time {
            let f = now.duration_since(t).as_millis() as f32 / c.speed;
            ease -= invoke(c.ease, f.clamp(0.0, 1.0), c.arg)?;
        }
        Some(ease)
    }

    /// `f`: time since `fadeOutStart` over the fade-out time.
    fn fade_seconds(&self, now: Instant, over_ms: f32) -> f32 {
        self.fade_out
            .map_or(-1.0, |t| now.duration_since(t).as_millis() as f32 / over_ms)
    }
}

thread_local! {
    static QUEUE: RefCell<VecDeque<Overlay>> = const { RefCell::new(VecDeque::new()) };
    static PREVIEW: RefCell<Option<Overlay>> = const { RefCell::new(None) };
    /// The settings page's open section (None: the section list).
    static SECTION: Cell<Option<usize>> = const { Cell::new(None) };
    /// The section's values when it opened ("Undo").
    static UNDO: RefCell<Vec<(&'static str, String)>> = const { RefCell::new(Vec::new()) };
    /// The `started` of the reload being applied, and of a recovery reload.
    static APPLYING: Cell<Option<Instant>> = const { Cell::new(None) };
    static RECOVERY: Cell<Option<Instant>> = const { Cell::new(None) };
}

fn ready(r: &super::PackReload) -> bool {
    r.started.elapsed() >= r.hold && r.handle.as_ref().is_some_and(|t| t.is_finished())
}

fn hides(game: &crate::game::Game<'_>, c: &Cfg) -> bool {
    game.native_on(ID) && c.can_hide(true)
}

/// A fresh reload goes out of sight before its overlay is drawn.
pub(crate) fn hide(game: &mut crate::game::Game<'_>) {
    let c = cfg();
    if !hides(game, &c) {
        return;
    }
    if let Some(reload) = game.pack_reload.take() {
        let now = Instant::now();
        QUEUE.with(|q| {
            let mut q = q.borrow_mut();
            // `setOverlay(new LoadingOverlay(..))`: one that only fades goes.
            q.retain(|o| !o.applied);
            q.push_back(Overlay::new(Some(reload), false, now));
        });
    }
}

/// Before the game drives its reload: the oldest held one goes back once done
/// (the game applies it in this same call, `onFinish`), else a fresh one is
/// hidden. Turned off (or not hiding reloads), held reloads go back at once.
pub(crate) fn drive(game: &mut crate::game::Game<'_>) {
    if game.pack_reload.is_none() {
        let c = cfg();
        let hiding = hides(game, &c);
        let back = QUEUE.with(|q| {
            let mut q = q.borrow_mut();
            let o = q.iter_mut().find(|o| o.reload.is_some())?;
            if hiding && !o.reload.as_ref().is_some_and(ready) {
                return None;
            }
            o.applied = true;
            o.fade_out = Some(Instant::now());
            o.reload.take()
        });
        if back.is_some() {
            game.pack_reload = back;
            APPLYING.set(game.pack_reload.as_ref().map(|r| r.started));
            return;
        }
    }
    hide(game);
    APPLYING.set(game.pack_reload.as_ref().map(|r| r.started));
}

/// The Options page this mod's settings are on.
fn on_page(game: &crate::game::Game<'_>) -> bool {
    use horizon_client::options_ui::Page;
    game.screen == crate::game::Screen::Options
        && matches!(game.ui.options.page(), Page::NativeMod(i)
            if game.settings.mods.installed.get(i).is_some_and(|m| m.id == ID))
}

/// Each frame: hide a fresh reload, move the overlays on (`currentProgress`),
/// and drop the ones done fading (`setOverlay(null)` once `f >= 2` and finished).
pub(crate) fn frame(game: &mut crate::game::Game<'_>) {
    let now = Instant::now();
    watch(now);
    let on = game.native_on(ID);
    let c = cfg();
    horizon_client::server_packs::rrls_early::EARLY.store(on && c.early, Ordering::Relaxed);
    if !on_page(game) {
        SECTION.set(None);
        PREVIEW.with(|p| *p.borrow_mut() = None);
    }
    hide(game);
    // A reload shown as the game's own overlay: RRLS sets its fade-in time to
    // half the animation speed, so it waits `animationSpeed` ms before applying.
    if on && c.speed.is_finite() && c.speed >= 0.0 && c.speed < 600_000.0 {
        if let Some(r) = game.pack_reload.as_mut() {
            r.hold = Duration::from_secs_f32(c.speed / 1000.0);
        }
    }
    QUEUE.with(|q| {
        let mut q = q.borrow_mut();
        for o in q.iter_mut() {
            o.update(now);
        }
        while q.len() > 1 && q.front().is_some_and(|o| o.applied) {
            q.pop_front();
        }
        if q.front().is_some_and(|o| {
            o.applied && (o.finished || !c.mini) && o.fade_seconds(now, 1000.0) >= 2.0
        }) {
            q.pop_front();
        }
    });
    PREVIEW.with(|p| {
        let mut p = p.borrow_mut();
        if let Some(o) = p.as_mut() {
            o.update(now);
            let done = if c.can_hide(false) {
                (o.finished || !c.mini) && o.fade_seconds(now, 1000.0) >= 2.0
            } else {
                o.fade_seconds(now, c.speed) >= 2.0
            };
            if done {
                *p = None;
            }
        }
    });
}

// ---------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------

fn start() -> Instant {
    static START: OnceLock<Instant> = OnceLock::new();
    *START.get_or_init(Instant::now)
}

fn c8(x: f32) -> u8 {
    (x * 255.0).floor().clamp(0.0, 255.0) as u8
}

/// `RainbowUtils.rainbowColor(alpha)`.
fn rainbow(alpha: f32) -> u32 {
    let time = start().elapsed().as_secs_f32();
    let ch = |offset: f32| (1.0 + (0.5 * time + offset).sin()) * 0.5;
    rgba(c8(ch(0.0)), c8(ch(2.0)), c8(ch(4.0)), c8(alpha))
}

fn white(alpha: i32) -> u32 {
    rgba(255, 255, 255, alpha.clamp(0, 255) as u8)
}

/// `GuiGraphicsExtractor.fill(x0, y0, x1, y1, color)`.
fn fill(batch: &mut HudBatch, atlas: &HudAtlas, [x0, y0, x1, y1]: [i32; 4], color: u32) {
    let (l, r) = (x0.min(x1), x0.max(x1));
    let (t, b) = (y0.min(y1), y0.max(y1));
    batch.fill(atlas, l as f32, t as f32, (r - l) as f32, (b - t) as f32, color);
}

/// `LoadingOverlay.extractProgressBar` with RRLS's alpha (`rrls$lerp`) and colours.
fn progress_bar(
    batch: &mut HudBatch,
    atlas: &HudAtlas,
    [x0, y0, x1, y1]: [i32; 4],
    progress: f32,
    fade: f32,
    rgb: bool,
) {
    let filled = (((x1 - x0 - 2) as f32) * progress).ceil() as i32;
    let alpha = (fade * 255.0 + 0.5).floor() as i32;
    let k = if rgb { rainbow(fade) } else { white(alpha) };
    fill(batch, atlas, [x0 + 2, y0 + 2, x0 + filled, y1 - 2], k);
    fill(batch, atlas, [x0 + 1, y0, x1 - 1, y0 + 1], k);
    fill(batch, atlas, [x0 + 1, y1, x1 - 1, y1 - 1], k);
    fill(batch, atlas, [x0, y0, x0 + 1, y1], k);
    fill(batch, atlas, [x1, y0, x1 - 1, y1], k);
}

fn text_of(s: &str) -> Text {
    if s.contains('§') { Text::legacy(s) } else { Text::literal(s) }
}

/// `LoadingOverlay.rrls$miniRender`.
fn mini(batch: &mut HudBatch, atlas: &HudAtlas, c: &Cfg, o: &mut Overlay, now: Instant) {
    let (i, j) = (batch.width as i32, batch.height as i32);
    let Some(ease) = o.ease(c, now) else {
        return;
    };
    if ease <= 0.0 {
        o.finished = true;
    }
    let ease_alpha = (4.0 + ease * 251.0).ceil() as i32;
    let ease_color = white(ease_alpha);
    match c.kind {
        // PROGRESS
        0 => {
            let s = (f64::from(j) * 0.8325) as i32;
            let r = ((f64::from(i) * 0.75).min(f64::from(j)) * 0.5) as i32;
            progress_bar(batch, atlas, [i / 2 - r, s - 5, i / 2 + r, s + 5], o.progress, ease, c.rgb);
        }
        // TEXT: centred at y 70.
        1 => {
            let t = text_of(&c.text);
            let w = atlas.font.width_text(&t) as i32;
            let color = if c.rgb { rainbow(ease_alpha as f32 / 255.0) } else { ease_color };
            batch.text_styled(atlas, &t, (i / 2 - w / 2) as f32, 70.0, color, true);
        }
        // TEXT_WITH_BACKGROUND: a FocusableTextWidget (padding 4, background
        // always, border) a third of the height up from the bottom.
        2 => {
            let t = text_of(&c.text);
            let w = (atlas.font.width_text(&t) as i32).min(i);
            let (x, y) = (i / 2 - w / 2, j - j / 3);
            let a = ease_alpha as f32 / 255.0;
            let (bx, by, bw, bh) = (x - 4, y - 4, w + 8, 9 + 8);
            let bg = rgba(0, 0, 0, (a * 85.0).floor().clamp(0.0, 255.0) as u8);
            fill(batch, atlas, [bx + 1, by, bx + bw, by + bh], bg);
            let border = rgba(0xA0, 0xA0, 0xA0, c8(a));
            fill(batch, atlas, [bx, by, bx + bw, by + 1], border);
            fill(batch, atlas, [bx, by + bh - 1, bx + bw, by + bh], border);
            fill(batch, atlas, [bx, by + 1, bx + 1, by + bh - 1], border);
            fill(batch, atlas, [bx + bw - 1, by + 1, bx + bw, by + bh - 1], border);
            let color = if c.rgb { rainbow(a) } else { ease_color };
            batch.text_styled(atlas, &t, x as f32, y as f32, color, true);
        }
        _ => {}
    }
}

/// The game's own overlay for the preview when RRLS would not hide it
/// (`LoadingOverlay.extractRenderState`, `fadeIn` false): opaque, then it
/// fades out over the animation speed once done.
fn full(batch: &mut HudBatch, atlas: &HudAtlas, c: &Cfg, o: &Overlay, now: Instant, dark: bool) {
    use horizon_client::level_loading::{MOJANG_STUDIOS_LOGO, brand_background, logo_layout};
    let (w, h) = (batch.width, batch.height);
    let f = o.fade_seconds(now, c.speed);
    let fade = if f >= 1.0 { 1.0 - (f - 1.0).clamp(0.0, 1.0) } else { 1.0 };
    let bg = brand_background(dark);
    let alpha = (fade * 255.0).ceil().clamp(0.0, 255.0) as u32;
    batch.fill(atlas, 0.0, 0.0, w, h, (bg & 0x00FF_FFFF) | alpha << 24);
    let (x, y, half_width, logo_height) = logo_layout(w, h);
    if let Some(r) = atlas.sprites.get(MOJANG_STUDIOS_LOGO).copied() {
        let at = |u: f32, v: f32| {
            let (u, v) = ((u / 120.0).clamp(0.0, 1.0), v / 120.0);
            (r.u0 + (r.u1 - r.u0) * u, r.v0 + (r.v1 - r.v0) * v)
        };
        for (n, (u, v)) in [(-0.0625, 0.0), (0.0625, 60.0)].into_iter().enumerate() {
            let (u0, v0) = at(u, v);
            let (u1, v1) = at(u + 120.0, v + 60.0);
            let half = horizon_client::hud::Rect { u0, v0, u1, v1, ..r };
            let left = x + n as i32 * half_width;
            batch.rect_uv(left as f32, y as f32, half_width as f32, logo_height as f32, half, white(alpha as i32));
        }
    }
    if f < 1.0 {
        let (i, j) = (w as i32, h as i32);
        let s = (f64::from(j) * 0.8325) as i32;
        let r = ((f64::from(i) * 0.75).min(f64::from(j)) * 0.5) as i32;
        let bar = 1.0 - f.clamp(0.0, 1.0);
        progress_bar(batch, atlas, [i / 2 - r, s - 5, i / 2 + r, s + 5], o.progress, bar, false);
    }
}

/// After the whole GUI (`Gui.extractRenderState`, before the subtitles): the
/// hidden overlay's mini version, or the settings section's preview.
pub(crate) fn draw(game: &crate::game::Game<'_>, batch: &mut HudBatch) {
    if !game.native_on(ID) {
        return;
    }
    let c = cfg();
    let atlas = &game.gui.atlas;
    let now = Instant::now();
    let drawn = QUEUE.with(|q| {
        let mut q = q.borrow_mut();
        let Some(o) = q.front_mut() else {
            return false;
        };
        if c.mini {
            mini(batch, atlas, &c, o, now);
        }
        true
    });
    if drawn {
        return;
    }
    PREVIEW.with(|p| {
        let mut p = p.borrow_mut();
        let Some(o) = p.as_mut() else {
            return;
        };
        if c.can_hide(false) {
            if c.mini {
                mini(batch, atlas, &c, o, now);
            }
        } else {
            let dark = game.settings.file.bool("darkMojangStudiosBackground");
            full(batch, atlas, &c, o, now, dark);
        }
    });
}

// ---------------------------------------------------------------------------------------------
// Options > Mods > Remove Reloading Screen (ConfigurationScreen and its sections)
// ---------------------------------------------------------------------------------------------

fn label(e: &Entry) -> String {
    let v = get(e.key);
    let shown = match e.kind {
        Kind::Bool => if v == "true" { "ON".to_owned() } else { "OFF".to_owned() },
        Kind::Enum(names) => names
            .iter()
            .find(|(n, _)| *n == v)
            .map_or(v.clone(), |(_, d)| (*d).to_owned()),
        Kind::Text | Kind::Number(_) => v,
    };
    format!("{}: {}", e.caption, shown)
}

fn section_entries(s: usize) -> impl Iterator<Item = &'static Entry> {
    ENTRIES.iter().filter(move |e| e.section == s)
}

/// The page's buttons: the sections ("Global..."), or a section's values with
/// Undo, Reset and Done.
pub(crate) fn settings(buttons: &mut Vec<String>) {
    match SECTION.get() {
        None => buttons.extend(SECTIONS.iter().map(|(_, name)| format!("{name}..."))),
        Some(s) => {
            buttons.extend(section_entries(s).map(label));
            buttons.extend(["Undo".to_owned(), "Reset".to_owned(), "Done".to_owned()]);
        }
    }
}

/// `ConfigurationSectionScreenWithOverlay.resetProgress`: the preview starts again.
fn reset_preview() {
    PREVIEW.with(|p| *p.borrow_mut() = Some(Overlay::new(None, true, Instant::now())));
}

fn open_section(s: usize) {
    SECTION.set(Some(s));
    UNDO.with(|u| *u.borrow_mut() = section_entries(s).map(|e| (e.key, get(e.key))).collect());
    reset_preview();
}

fn step(e: &Entry, back: bool) -> Option<String> {
    let v = get(e.key);
    match e.kind {
        Kind::Bool => Some(if v == "true" { "false".into() } else { "true".into() }),
        Kind::Enum(names) => {
            let i = names.iter().position(|(n, _)| *n == v).unwrap_or(0);
            let n = names.len();
            let next = if back { (i + n - 1) % n } else { (i + 1) % n };
            Some(names[next].0.to_owned())
        }
        Kind::Number(values) => {
            let cur = parse_num(&v).unwrap_or(f64::NAN);
            let at = values.iter().position(|x| x == &cur || (x.is_nan() && cur.is_nan()));
            let n = values.len();
            let next = match at {
                Some(i) if back => (i + n - 1) % n,
                Some(i) => (i + 1) % n,
                // A value set in the file: the next listed one above (below) it.
                None if back => values.iter().rposition(|x| *x < cur).unwrap_or(n - 1),
                None => values.iter().position(|x| *x > cur).unwrap_or(0),
            };
            Some(fmt_num(values[next]))
        }
        // Edited in the config file ("Edit in config!"): opened for the player.
        Kind::Text => {
            if let Some(p) = path() {
                if !p.exists() {
                    with_values(|v| write_file(&v.map));
                }
                let _ = std::process::Command::new("notepad.exe").arg(&p).spawn();
            }
            None
        }
    }
}

pub(crate) fn click(game: &mut crate::game::Game<'_>, button: usize) {
    let back = game.modifiers.shift_key();
    let Some(s) = SECTION.get() else {
        if button < SECTIONS.len() {
            open_section(button);
        }
        return;
    };
    let entries: Vec<&Entry> = section_entries(s).collect();
    match button.checked_sub(entries.len()) {
        None => {
            let e = entries[button];
            if let Some(v) = step(e, back) {
                set(e.key, v);
                reset_preview();
            }
        }
        // Undo
        Some(0) => {
            for (k, v) in UNDO.with(|u| u.borrow().clone()) {
                set(k, v);
            }
            reset_preview();
        }
        // Reset
        Some(1) => {
            for e in &entries {
                set(e.key, e.default.to_owned());
            }
            reset_preview();
        }
        // Done
        _ => {
            SECTION.set(None);
            PREVIEW.with(|p| *p.borrow_mut() = None);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// A failed reload (Minecraft.clearResourcePacksOnError with RRLS's resetResources and doubleLoad)
// ---------------------------------------------------------------------------------------------

fn load_fail_toast(game: &mut crate::game::Game<'_>) {
    let title = horizon_client::lang::tr("resourcePack.load_fail", &[]);
    match &mut game.server_packs.load_fail {
        Some(toast) => toast.reset(title, None),
        None => {
            if game.toast_allowed("system", "pack_load_failure") {
                game.server_packs.load_fail =
                    Some(horizon_client::toasts::SystemToast::new(title, None));
            }
        }
    }
}

/// Before the game handles a failed reload of the player's packs. With the
/// defaults (reset, force load) the game's own handling runs unchanged.
pub(crate) fn reload_failed(game: &mut crate::game::Game<'_>, error: &str) -> ControlFlow<()> {
    if !game.native_on(ID) {
        return ControlFlow::Continue(());
    }
    let c = cfg();
    // The recovery reload failed as well: the game's own handling.
    let recovering = RECOVERY.get().is_some() && RECOVERY.get() == APPLYING.get();
    if recovering || (c.reset_resources && c.double_load == 0) {
        RECOVERY.set(None);
        return ControlFlow::Continue(());
    }
    log::warn!("Horizon: resource reload failed: {error}");
    if !c.reset_resources {
        // rrls$onResourceReloadFailure: the packs stay, and load again.
        log::error!("Caught error loading resource packs! {error}");
        if c.double_load != 2 && game.spawn_pack_reload(super::ReloadKind::Selection).is_ok() {
            RECOVERY.set(game.pack_reload.as_ref().map(|r| r.started));
            load_fail_toast(game);
            hide(game);
        }
        return ControlFlow::Break(());
    }
    // resetResources with doubleLoad NONE: the packs are cleared, nothing reloads.
    let all = super::packs::discover(&game.vanilla_root(), &super::packs_dir());
    if super::packs::able_to_clear_any_pack(&all, &game.settings.resource_packs) {
        log::info!("Caught error loading resourcepacks, removing all selected resourcepacks");
        game.settings.resource_packs.clear();
        game.settings.save(&crate::game::Game::options_path());
        return ControlFlow::Break(());
    }
    ControlFlow::Continue(())
}
