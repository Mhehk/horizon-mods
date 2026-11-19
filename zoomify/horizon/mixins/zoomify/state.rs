// Zoomify — mhv1 (mihehk): a Horizon port of isXander's Zoomify
// (https://github.com/isXander/Zoomify), licensed like the original under the
// GNU Lesser General Public License v3 or later (LICENSE beside this mod).
//
// The zoom key narrows the field of view with eased transitions, the scroll
// wheel zooms further in steps, a secondary zoom (toggle) zooms slowly and can
// hide the HUD, and the spyglass works with it. The settings are Zoomify's own
// (`config/zoomify.json`, same keys and defaults), on Options > Mods > Zoomify
// (or `/zoomify`), in its categories: Behavior, Controls, Secondary Zoom, Misc.
use core::ops::ControlFlow;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

pub(crate) const ID: &str = "zoomify";

// ---------------------------------------------------------------------------------------------
// TransitionType
// ---------------------------------------------------------------------------------------------

const TRANSITIONS: &[(&str, &str)] = &[
    ("instant", "Instant"),
    ("linear", "Linear"),
    ("ease_in_sine", "Ease In Sine"),
    ("ease_out_sine", "Ease Out Sine"),
    ("ease_in_out_sine", "Ease In Out Sine"),
    ("ease_in_quad", "Ease In Quad"),
    ("ease_out_quad", "Ease Out Quad"),
    ("ease_in_out_quad", "Ease In Out Quad"),
    ("ease_in_cubic", "Ease In Cubic"),
    ("ease_out_cubic", "Ease Out Cubic"),
    ("ease_in_out_cubic", "Ease In Out Cubic"),
    ("ease_in_exp", "Ease In Exponential"),
    ("ease_out_exp", "Ease Out Exponential"),
    ("ease_in_out_exp", "Ease In Out Exponential"),
];

#[derive(Clone, Copy, PartialEq, Debug)]
struct Transition(usize);

impl Transition {
    const INSTANT: Transition = Transition(0);

    fn named(name: &str) -> Self {
        Transition(TRANSITIONS.iter().position(|(n, _)| *n == name).unwrap_or(12))
    }
    fn name(self) -> &'static str {
        TRANSITIONS[self.0].0
    }
    fn apply(self, t: f64) -> f64 {
        use std::f64::consts::PI;
        let log2_1023 = 1023f64.log2();
        // Kotlin's `1/1023` is an integer division: nothing is subtracted.
        match self.name() {
            "ease_in_sine" => 1.0 - ((t * PI) / 2.0).cos(),
            "ease_out_sine" => ((t * PI) / 2.0).sin(),
            "ease_in_out_sine" => -((PI * t).cos() - 1.0) / 2.0,
            "ease_in_quad" => t * t,
            "ease_out_quad" => 1.0 - (1.0 - t) * (1.0 - t),
            "ease_in_out_quad" => {
                if t < 0.5 { 2.0 * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 }
            }
            "ease_in_cubic" => t.powi(3),
            "ease_out_cubic" => 1.0 - (1.0 - t).powi(3),
            "ease_in_out_cubic" => {
                if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
            }
            "ease_in_exp" => {
                if t == 0.0 || t == 1.0 { t } else { 2f64.powf(10.0 * t - log2_1023) }
            }
            "ease_out_exp" => {
                if t == 0.0 || t == 1.0 { t } else { 1.0 - 2f64.powf(10.0 - log2_1023 - 10.0 * t) }
            }
            "ease_in_out_exp" => {
                if t == 0.0 || t == 1.0 {
                    t
                } else if t < 0.5 {
                    2f64.powf(20.0 * t - log2_1023)
                } else {
                    1.0 - 2f64.powf(10.0 - log2_1023 - 10.0 * t)
                }
            }
            _ => t,
        }
    }
    fn inverse(self, x: f64) -> Option<f64> {
        use std::f64::consts::PI;
        let ln2_10 = 10.0 * 2f64.ln();
        Some(match self.name() {
            "ease_in_sine" => (-(x - 1.0)).acos() * 2.0 / PI,
            "ease_out_sine" => x.asin() * 2.0 / PI,
            "ease_in_quad" => x.sqrt(),
            "ease_out_quad" => -((-(x - 1.0)).sqrt() - 1.0),
            "ease_in_cubic" => x.powf(1.0 / 3.0),
            "ease_out_cubic" => -((-x + 1.0).powf(1.0 / 3.0)) + 1.0,
            "ease_in_exp" => {
                if x == 0.0 || x == 1.0 { x } else { (1023.0 * x + 1.0).ln() / ln2_10 }
            }
            "ease_out_exp" => {
                if x == 0.0 || x == 1.0 {
                    x
                } else {
                    -(((-((1023.0 * x - 1024.0) / 1023.0)).ln() - ln2_10 + 1023f64.ln()) / ln2_10)
                }
            }
            _ => return None,
        })
    }
    fn has_inverse(self) -> bool {
        self.inverse(0.0).is_some()
    }
    fn opposite(self) -> Self {
        let n = self.name();
        let o = if let Some(rest) = n.strip_prefix("ease_in_out_") {
            format!("ease_in_out_{rest}")
        } else if let Some(rest) = n.strip_prefix("ease_in_") {
            format!("ease_out_{rest}")
        } else if let Some(rest) = n.strip_prefix("ease_out_") {
            format!("ease_in_{rest}")
        } else {
            n.to_owned()
        };
        Transition::named(&o)
    }
}

// ---------------------------------------------------------------------------------------------
// ZoomifySettings (config/zoomify.json)
// ---------------------------------------------------------------------------------------------

const SPYGLASS: &[(&str, &str)] = &[
    ("combine", "Combine"),
    ("override", "Override"),
    ("only_zoom_while_holding", "Allow While Holding"),
    ("only_zoom_while_carrying", "Allow While Carrying"),
];
const OVERLAY: &[(&str, &str)] = &[
    ("never", "Never"),
    ("holding", "Holding"),
    ("carrying", "Carrying"),
    ("always", "Always"),
];
const SOUND: &[(&str, &str)] = &[
    ("never", "Never"),
    ("always", "Always"),
    ("only_spyglass", "Only Holding Spyglass"),
    ("with_overlay", "Match Overlay"),
];
const KEY_BEHAVIOUR: &[(&str, &str)] = &[("hold", "Hold"), ("toggle", "Toggle")];

#[derive(Clone, Copy)]
enum Fmt {
    Plain,
    /// `%dx`
    Times,
    /// `%.1fx` of the value over 100
    TimesTenth,
    /// `%.1f secs`
    Secs,
    /// 0 "Instant", else `%.2f secs`
    SecsInstant,
    /// 0 "Instant", else `%d%%`
    PercentInstant,
    /// 0 "OFF", else `%d%%`
    PercentOff,
}

#[derive(Clone, Copy)]
enum Kind {
    Bool,
    Enum(&'static [(&'static str, &'static str)]),
    Int(i64, i64, i64, Fmt),
    Double(f64, f64, f64, Fmt),
}

struct Entry {
    section: usize,
    key: &'static str,
    caption: &'static str,
    kind: Kind,
    default: &'static str,
}

const SECTIONS: [&str; 4] = ["Behavior", "Controls", "Secondary Zoom", "Misc"];

const ENTRIES: &[Entry] = &[
    Entry { section: 0, key: "initialZoom", caption: "Initial Zoom", kind: Kind::Int(1, 10, 1, Fmt::Times), default: "4" },
    Entry { section: 0, key: "zoomInTime", caption: "Zoom In Time", kind: Kind::Double(0.1, 5.0, 0.1, Fmt::Secs), default: "1.0" },
    Entry { section: 0, key: "zoomOutTime", caption: "Zoom Out Time", kind: Kind::Double(0.1, 5.0, 0.1, Fmt::Secs), default: "0.5" },
    Entry { section: 0, key: "zoomInTransition", caption: "Zoom In Transition", kind: Kind::Enum(TRANSITIONS), default: "ease_out_exp" },
    Entry { section: 0, key: "zoomOutTransition", caption: "Zoom Out Transition", kind: Kind::Enum(TRANSITIONS), default: "ease_out_exp" },
    Entry { section: 0, key: "affectHandFov", caption: "Affect Hand FOV", kind: Kind::Bool, default: "true" },
    Entry { section: 0, key: "scrollZoom", caption: "Enable Scroll Zoom", kind: Kind::Bool, default: "true" },
    Entry { section: 0, key: "scrollStepCount", caption: "Scroll Step Count", kind: Kind::Int(3, 20, 1, Fmt::Plain), default: "10" },
    Entry { section: 0, key: "zoomPerStep", caption: "Zoom Per Step", kind: Kind::Int(110, 200, 10, Fmt::TimesTenth), default: "150" },
    Entry { section: 0, key: "scrollZoomSmoothness", caption: "Scroll Zoom Smoothness", kind: Kind::Int(0, 100, 1, Fmt::PercentInstant), default: "70" },
    Entry { section: 0, key: "retainZoomSteps", caption: "Remember Zoom Steps", kind: Kind::Bool, default: "false" },
    Entry { section: 0, key: "spyglassBehaviour", caption: "Zoom Behavior", kind: Kind::Enum(SPYGLASS), default: "combine" },
    Entry { section: 0, key: "spyglassOverlayVisibility", caption: "Overlay Visibility", kind: Kind::Enum(OVERLAY), default: "holding" },
    Entry { section: 0, key: "spyglassSoundBehaviour", caption: "Sound Behavior", kind: Kind::Enum(SOUND), default: "with_overlay" },
    Entry { section: 1, key: "zoomKeyBehaviour", caption: "Zoom Key Behavior", kind: Kind::Enum(KEY_BEHAVIOUR), default: "hold" },
    Entry { section: 1, key: "_keybindScrolling", caption: "Keybind Scrolling", kind: Kind::Bool, default: "false" },
    Entry { section: 1, key: "relativeSensitivity", caption: "Relative Sensitivity", kind: Kind::Int(0, 150, 10, Fmt::PercentOff), default: "100" },
    Entry { section: 1, key: "relativeViewBobbing", caption: "Relative View Bobbing", kind: Kind::Bool, default: "true" },
    Entry { section: 1, key: "cinematicCamera", caption: "Cinematic Camera", kind: Kind::Int(0, 250, 10, Fmt::PercentOff), default: "0" },
    Entry { section: 2, key: "secondaryZoomAmount", caption: "Zoom Amount", kind: Kind::Int(2, 10, 1, Fmt::Times), default: "4" },
    Entry { section: 2, key: "secondaryZoomInTime", caption: "Zoom In Time", kind: Kind::Double(6.0, 30.0, 2.0, Fmt::Secs), default: "10.0" },
    Entry { section: 2, key: "secondaryZoomOutTime", caption: "Zoom Out Time", kind: Kind::Double(0.0, 5.0, 0.25, Fmt::SecsInstant), default: "1.0" },
    Entry { section: 2, key: "secondaryHideHUDOnZoom", caption: "Hide HUD On Zoom", kind: Kind::Bool, default: "true" },
    // Not on the screen: set false once the game has started with Zoomify.
    Entry { section: 9, key: "_firstLaunch", caption: "", kind: Kind::Bool, default: "true" },
];

/// The scrolling options are only available with scroll zoom on.
const NEEDS_SCROLL_ZOOM: [&str; 4] = ["scrollStepCount", "zoomPerStep", "scrollZoomSmoothness", "retainZoomSteps"];

fn fmt_double(v: f64) -> String {
    format!("{v:?}")
}

fn normalize(e: &Entry, v: &serde_json::Value) -> Option<String> {
    match e.kind {
        Kind::Bool => v.as_bool().map(|b| b.to_string()),
        Kind::Enum(names) => {
            let s = v.as_str()?;
            names.iter().any(|(n, _)| *n == s).then(|| s.to_owned())
        }
        Kind::Int(..) => v.as_i64().map(|i| i.to_string()),
        Kind::Double(..) => v.as_f64().map(fmt_double),
    }
}

struct Values {
    map: BTreeMap<&'static str, String>,
    stamp: Option<SystemTime>,
    checked: Instant,
    /// `keybindScrolling`: read once when the game starts (a restart applies it).
    keybind_scrolling: bool,
}

thread_local! {
    static VALUES: RefCell<Option<Values>> = const { RefCell::new(None) };
}

fn path() -> Option<PathBuf> {
    crate::game::Game::native_config_dir()
        .ok()
        .map(|d| d.join("zoomify.json"))
}

fn stamp() -> Option<SystemTime> {
    std::fs::metadata(path()?).and_then(|m| m.modified()).ok()
}

fn defaults() -> BTreeMap<&'static str, String> {
    ENTRIES.iter().map(|e| (e.key, e.default.to_owned())).collect()
}

/// `loadFromFile`: false when there is no readable file.
fn read_file(map: &mut BTreeMap<&'static str, String>) -> bool {
    let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) else {
        return false;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        log::error!(target: "Zoomify", "Failed to read zoomify.json");
        return false;
    };
    for e in ENTRIES {
        if let Some(v) = json.get(e.key).and_then(|v| normalize(e, v)) {
            map.insert(e.key, v);
        }
    }
    true
}

fn write_file(map: &BTreeMap<&'static str, String>) {
    let Some(path) = path() else {
        return;
    };
    let lines: Vec<String> = ENTRIES
        .iter()
        .map(|e| {
            let v = map.get(e.key).cloned().unwrap_or_else(|| e.default.into());
            let v = match e.kind {
                Kind::Enum(_) => format!("\"{v}\""),
                _ => v,
            };
            format!("  \"{}\": {}", e.key, v)
        })
        .collect();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, format!("{{\n{}\n}}", lines.join(",\n")));
}

fn with_values<R>(f: impl FnOnce(&mut Values) -> R) -> R {
    VALUES.with(|cell| {
        let mut cell = cell.borrow_mut();
        let v = cell.get_or_insert_with(|| {
            let mut map = defaults();
            if !read_file(&mut map) {
                write_file(&map);
            }
            if map.get("_firstLaunch").is_some_and(|v| v == "true") {
                map.insert("_firstLaunch", "false".into());
                write_file(&map);
            }
            let keybind_scrolling = map.get("_keybindScrolling").is_some_and(|v| v == "true");
            Values { map, stamp: stamp(), checked: Instant::now(), keybind_scrolling }
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

fn watch(now: Instant) {
    with_values(|v| {
        if now.duration_since(v.checked) < Duration::from_secs(1) {
            return;
        }
        v.checked = now;
        let s = stamp();
        if s != v.stamp {
            v.stamp = s;
            let mut map = defaults();
            read_file(&mut map);
            v.map = map;
        }
    });
}

/// The values the zoom reads.
#[derive(Clone)]
struct Cfg {
    initial_zoom: i64,
    zoom_in_time: f64,
    zoom_out_time: f64,
    zoom_in_transition: Transition,
    zoom_out_transition: Transition,
    affect_hand_fov: bool,
    retain_zoom_steps: bool,
    scroll_zoom: bool,
    scroll_step_count: i64,
    zoom_per_step: i64,
    scroll_zoom_smoothness: i64,
    toggle: bool,
    keybind_scrolling: bool,
    relative_sensitivity: i64,
    relative_view_bobbing: bool,
    cinematic_camera: i64,
    spyglass: usize,
    overlay: usize,
    sound: usize,
    secondary_amount: i64,
    secondary_in: f64,
    secondary_out: f64,
    secondary_hide_hud: bool,
}

fn cfg() -> Cfg {
    with_values(|v| {
        let s = |k: &str| v.map.get(k).cloned().unwrap_or_default();
        let i = |k: &str| s(k).parse::<i64>().unwrap_or(0);
        let d = |k: &str| s(k).parse::<f64>().unwrap_or(0.0);
        let b = |k: &str| s(k) == "true";
        let e = |k: &str, names: &[(&str, &str)]| {
            let x = s(k);
            names.iter().position(|(n, _)| *n == x).unwrap_or(0)
        };
        Cfg {
            initial_zoom: i("initialZoom"),
            zoom_in_time: d("zoomInTime"),
            zoom_out_time: d("zoomOutTime"),
            zoom_in_transition: Transition::named(&s("zoomInTransition")),
            zoom_out_transition: Transition::named(&s("zoomOutTransition")),
            affect_hand_fov: b("affectHandFov"),
            retain_zoom_steps: b("retainZoomSteps"),
            scroll_zoom: b("scrollZoom"),
            scroll_step_count: i("scrollStepCount"),
            zoom_per_step: i("zoomPerStep"),
            scroll_zoom_smoothness: i("scrollZoomSmoothness"),
            toggle: s("zoomKeyBehaviour") == "toggle",
            keybind_scrolling: v.keybind_scrolling,
            relative_sensitivity: i("relativeSensitivity"),
            relative_view_bobbing: b("relativeViewBobbing"),
            cinematic_camera: i("cinematicCamera"),
            spyglass: e("spyglassBehaviour", SPYGLASS),
            overlay: e("spyglassOverlayVisibility", OVERLAY),
            sound: e("spyglassSoundBehaviour", SOUND),
            secondary_amount: i("secondaryZoomAmount"),
            secondary_in: d("secondaryZoomInTime"),
            secondary_out: d("secondaryZoomOutTime"),
            secondary_hide_hud: b("secondaryHideHUDOnZoom"),
        }
    })
}

/// The scroll keys are only registered with keybind scrolling (on at start).
pub(crate) fn keys() -> Vec<[String; 3]> {
    let mut keys = vec![
        ["zoom".into(), "Zoom (Hold)".into(), "key.keyboard.c".into()],
        ["zoom.secondary".into(), "Secondary Zoom".into(), "key.keyboard.f6".into()],
    ];
    if with_values(|v| v.keybind_scrolling) {
        keys.push(["zoom.in".into(), "Scroll Zoom In".into(), "key.keyboard.unknown".into()]);
        keys.push(["zoom.out".into(), "Scroll Zoom Out".into(), "key.keyboard.unknown".into()]);
    }
    keys
}

// ---------------------------------------------------------------------------------------------
// Interpolators and ZoomHelper
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum InterpKind {
    /// TransitionInterpolator (the zoom key's in and out).
    Transition,
    /// SmoothInterpolator (the scroll steps).
    Smooth,
    /// TimedInterpolator (the secondary zoom).
    Timed,
    /// InstantInterpolator (the secondary zoom's scroll, none).
    Instant,
}

struct Interp {
    kind: InterpKind,
    going_in: bool,
    active: Transition,
    inactive: Transition,
    prev_target: f64,
    just_swapped: bool,
}

impl Interp {
    fn new(kind: InterpKind, c: &Cfg) -> Self {
        Self {
            kind,
            going_in: true,
            active: c.zoom_in_transition,
            // The constructor's own `transitionOut`, not its opposite.
            inactive: c.zoom_out_transition,
            prev_target: 0.0,
            just_swapped: false,
        }
    }

    fn times(&self, c: &Cfg, secondary: bool) -> (f64, f64) {
        if secondary { (c.secondary_in, c.secondary_out) } else { (c.zoom_in_time, c.zoom_out_time) }
    }

    fn smoothness(c: &Cfg) -> f64 {
        // Mth.lerp(smoothness / 100, 1.0, 0.1)
        1.0 + c.scroll_zoom_smoothness as f64 / 100.0 * (0.1 - 1.0)
    }

    fn is_smooth(&self, c: &Cfg, secondary: bool) -> bool {
        let (tin, tout) = self.times(c, secondary);
        let timed = (self.going_in && tin > 0.0) || (!self.going_in && tout > 0.0);
        match self.kind {
            InterpKind::Instant => false,
            InterpKind::Timed => timed,
            InterpKind::Smooth => Self::smoothness(c) != 1.0,
            InterpKind::Transition => !self.just_swapped && timed && self.active != Transition::INSTANT,
        }
    }

    fn increment(&self, c: &Cfg, secondary: bool, out: bool, dt: f64, target: f64, current: f64) -> f64 {
        match self.kind {
            InterpKind::Smooth => {
                let diff = if !out { target - current } else { current - target };
                diff * Self::smoothness(c) / 0.05 * dt
            }
            _ => {
                let (tin, tout) = self.times(c, secondary);
                dt / if out { tout } else { tin }
            }
        }
    }

    fn linear(&mut self, c: &Cfg, secondary: bool, target: f64, current: f64, dt: f64) -> f64 {
        if target > current {
            self.going_in = true;
            (current + self.increment(c, secondary, false, dt, target, current)).min(target)
        } else if target < current {
            self.going_in = false;
            (current - self.increment(c, secondary, true, dt, target, current)).max(target)
        } else {
            self.going_in = true;
            target
        }
    }

    fn tick(&mut self, c: &Cfg, secondary: bool, target: f64, current: f64, dt: f64) -> f64 {
        match self.kind {
            InterpKind::Instant => target,
            InterpKind::Timed => self.linear(c, secondary, target, current, dt),
            InterpKind::Smooth => {
                if !self.is_smooth(c, secondary) {
                    return target;
                }
                self.linear(c, secondary, target, current, dt)
            }
            InterpKind::Transition => {
                let tin = c.zoom_in_transition;
                let tout = c.zoom_out_transition.opposite();
                let mut cur = current;
                if target > current {
                    self.active = tin;
                    self.inactive = tout;
                    if self.prev_target < target && self.active.has_inverse() {
                        self.just_swapped = true;
                        cur = self.active.inverse(self.inactive.apply(cur)).unwrap_or(cur);
                    }
                } else if target < current {
                    self.active = tout;
                    self.inactive = tin;
                    if self.prev_target > target && self.active.has_inverse() {
                        self.just_swapped = true;
                        cur = self.active.inverse(self.inactive.apply(cur)).unwrap_or(cur);
                    }
                }
                self.prev_target = target;
                if self.active == Transition::INSTANT {
                    return target;
                }
                self.linear(c, secondary, target, cur, dt)
            }
        }
    }

    fn modify(&self, x: f64) -> f64 {
        match self.kind {
            InterpKind::Transition => self.active.apply(x),
            _ => x,
        }
    }

    fn modify_prev(&mut self, x: f64) -> f64 {
        if self.kind == InterpKind::Transition && self.just_swapped {
            self.just_swapped = false;
            return self.active.inverse(self.inactive.apply(x)).unwrap_or(x);
        }
        x
    }
}

/// `ZoomHelper`: the regular zoom or the secondary one.
struct Helper {
    secondary: bool,
    initial: Interp,
    scroll: Interp,
    prev_initial: f64,
    initial_t: f64,
    zooming_last_tick: bool,
    prev_scroll: f64,
    scroll_t: f64,
    last_scroll_tier: i64,
    resetting: bool,
    reset_multiplier: f64,
}

impl Helper {
    fn new(secondary: bool, c: &Cfg) -> Self {
        let (i, s) = if secondary {
            (InterpKind::Timed, InterpKind::Instant)
        } else {
            (InterpKind::Transition, InterpKind::Smooth)
        };
        Self {
            secondary,
            initial: Interp::new(i, c),
            scroll: Interp::new(s, c),
            prev_initial: 0.0,
            initial_t: 0.0,
            zooming_last_tick: false,
            prev_scroll: 0.0,
            scroll_t: 0.0,
            last_scroll_tier: 0,
            resetting: false,
            reset_multiplier: 0.0,
        }
    }

    fn initial_zoom(&self, c: &Cfg) -> f64 {
        if self.secondary { c.secondary_amount as f64 } else { c.initial_zoom as f64 }
    }
    fn zoom_per_step(&self, c: &Cfg) -> f64 {
        if self.secondary { 100.0 } else { c.zoom_per_step as f64 }
    }
    fn max_tiers(&self, c: &Cfg) -> i64 {
        if self.secondary { 0 } else { c.scroll_step_count }
    }

    /// Scrolling out stops at the normal field of view.
    fn min_tiers(&self, c: &Cfg) -> i64 {
        let step = self.zoom_per_step(c) / 100.0;
        if step <= 1.0 {
            return 0;
        }
        let steps = (self.initial_zoom(c).ln() / step.ln()).ceil() as i64;
        -steps.clamp(0, self.max_tiers(c))
    }

    fn tick(&mut self, c: &Cfg, zooming: bool, tiers: i64) {
        let dt = 0.05;
        // tickInitial
        if zooming && !self.zooming_last_tick {
            self.resetting = false;
        }
        let target = if zooming { 1.0 } else { 0.0 };
        self.prev_initial = self.initial_t;
        self.initial_t = self.initial.tick(c, self.secondary, target, self.initial_t, dt);
        self.prev_initial = self.initial.modify_prev(self.prev_initial);
        if !self.initial.is_smooth(c, self.secondary) {
            self.prev_initial = self.initial_t;
        }
        self.zooming_last_tick = zooming;
        // tickScroll
        if tiers.abs() > self.last_scroll_tier.abs() {
            self.resetting = false;
        }
        let max = self.max_tiers(c);
        let target = if max > 0 { tiers as f64 / max as f64 } else { 0.0 };
        self.prev_scroll = self.scroll_t;
        self.scroll_t = self.scroll.tick(c, self.secondary, target, self.scroll_t, dt);
        self.prev_scroll = self.scroll.modify_prev(self.prev_scroll);
        if !self.initial.is_smooth(c, self.secondary) {
            self.prev_initial = self.initial_t;
        }
        self.last_scroll_tier = tiers;
    }

    fn divisor(&mut self, c: &Cfg, partial: f64) -> f64 {
        let lerp = |d: f64, a: f64, b: f64| a + d * (b - a);
        let initial = if self.initial.is_smooth(c, self.secondary) {
            self.initial.modify(lerp(partial, self.prev_initial, self.initial_t))
        } else {
            self.initial_t
        };
        let end = if !self.resetting { 1.0 / self.initial_zoom(c) } else { self.reset_multiplier };
        let base = 1.0 / lerp(initial, 1.0, end);
        let scroll = if self.resetting {
            0.0
        } else if self.scroll.is_smooth(c, self.secondary) {
            self.scroll.modify(lerp(partial, self.prev_scroll, self.scroll_t))
        } else {
            self.scroll_t
        };
        let step = self.zoom_per_step(c) / 100.0;
        let current = scroll * self.max_tiers(c) as f64;
        let divisor = (base * step.powf(current)).clamp(1.0, 500.0);
        if self.initial_t == 0.0 && self.scroll_t == 0.0 {
            self.resetting = false;
        }
        if !self.resetting {
            self.reset_multiplier = 1.0 / divisor;
        }
        divisor
    }

    fn reset(&mut self) {
        if !self.resetting && self.scroll_t != 0.0 {
            self.resetting = true;
            self.scroll_t = 0.0;
            self.prev_scroll = 0.0;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Zoomify: keys, tick and the zoom divisor
// ---------------------------------------------------------------------------------------------

struct Zoom {
    zooming: bool,
    secondary_zooming: bool,
    previous_divisor: f64,
    scroll_steps: i64,
    should_play_sound: bool,
    display_gui: bool,
    regular: Helper,
    secondary: Helper,
    held: HashSet<&'static str>,
    clicks: Vec<&'static str>,
}

thread_local! {
    static ZOOM: RefCell<Option<Zoom>> = const { RefCell::new(None) };
    static IN_HOOK: Cell<bool> = const { Cell::new(false) };
    /// The settings page's open category (None: the category list).
    static SECTION: Cell<Option<usize>> = const { Cell::new(None) };
    static UNDO: RefCell<Vec<(&'static str, String)>> = const { RefCell::new(Vec::new()) };
}

fn with_zoom<R>(f: impl FnOnce(&mut Zoom, &Cfg) -> R) -> R {
    let c = cfg();
    ZOOM.with(|z| {
        let mut z = z.borrow_mut();
        let zoom = z.get_or_insert_with(|| Zoom {
            zooming: false,
            secondary_zooming: false,
            previous_divisor: 1.0,
            scroll_steps: 0,
            should_play_sound: false,
            display_gui: false,
            regular: Helper::new(false, &c),
            secondary: Helper::new(true, &c),
            held: HashSet::new(),
            clicks: Vec::new(),
        });
        f(zoom, &c)
    })
}

const KEY_NAMES: [&str; 4] = ["zoom", "zoom.secondary", "zoom.in", "zoom.out"];

fn default_key(name: &str) -> &'static str {
    match name {
        "zoom" => "key.keyboard.c",
        "zoom.secondary" => "key.keyboard.f6",
        _ => "key.keyboard.unknown",
    }
}

/// A key went down or up: Zoomify's mappings bound to it hold or click.
pub(crate) fn key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    if !game.native_on(ID) || !game.native_window() || key == "key.keyboard.unknown" {
        return;
    }
    let playing = game.native_playing();
    let names: Vec<&'static str> = KEY_NAMES
        .into_iter()
        .filter(|n| game.native_mod_key(ID, n, default_key(n)) == key)
        .collect();
    with_zoom(|z, _| {
        for n in names {
            if pressed {
                // KeyMapping.click: only in the world.
                if playing {
                    z.held.insert(n);
                    z.clicks.push(n);
                }
            } else {
                z.held.remove(n);
            }
        }
    });
}

fn consume(z: &mut Zoom, name: &str) -> usize {
    let n = z.clicks.iter().filter(|c| **c == name).count();
    z.clicks.retain(|c| *c != name);
    n
}

fn spyglass_in(stack: Option<&horizon_client::items::ItemStack>) -> bool {
    stack.is_some_and(|s| &*s.item == "minecraft:spyglass")
}

fn holding_spyglass(game: &crate::game::Game<'_>) -> bool {
    spyglass_in(game.inventory.get(game.selected).and_then(Option::as_ref)) || spyglass_in(game.offhand.as_ref())
}

fn carrying_spyglass(game: &crate::game::Game<'_>) -> bool {
    game.inventory.iter().any(|s| spyglass_in(s.as_ref())) || spyglass_in(game.offhand.as_ref())
}

/// `Zoomify.shouldRenderOverlay`.
fn should_render_overlay(game: &crate::game::Game<'_>, c: &Cfg, zooming: bool, using_spyglass: bool) -> bool {
    match c.overlay {
        0 => false,
        3 => zooming,
        1 => using_spyglass || (zooming && holding_spyglass(game) && c.spyglass != 0),
        _ => zooming && carrying_spyglass(game),
    }
}

fn play(game: &mut crate::game::Game<'_>, sound: &'static str) {
    let at = game.player.as_ref().map(|p| [p.position.x, p.position.y, p.position.z]);
    game.audio.play(sound, at, 1.0, 1.0);
}

/// `Zoomify.handleSpyglass`.
fn handle_spyglass(game: &mut crate::game::Game<'_>, z: &mut Zoom, c: &Cfg, prev: bool) {
    if game.player.is_none() {
        return;
    }
    let scoping = game.scoping();
    match c.spyglass {
        2 if !holding_spyglass(game) => z.zooming = false,
        3 if !carrying_spyglass(game) => z.zooming = false,
        1 if scoping => z.zooming = z.zooming && game.camera_type.first_person(),
        _ => {}
    }
    let requires = c.spyglass != 0;
    if requires && scoping {
        z.zooming = true;
    }
    if z.should_play_sound && !z.zooming && prev {
        play(game, "minecraft:item.spyglass.stop_using");
    }
    z.should_play_sound = match c.sound {
        0 => false,
        1 => true,
        2 => scoping || (requires && z.zooming && holding_spyglass(game)),
        _ => {
            should_render_overlay(game, c, z.zooming, game.camera_type.first_person() && scoping) && requires
        }
    };
    if z.should_play_sound && z.zooming && !prev {
        play(game, "minecraft:item.spyglass.use");
    }
}

fn open_settings(game: &mut crate::game::Game<'_>) {
    game.list_native_mods();
    if let Some(i) = game.settings.mods.installed.iter().position(|m| m.id == ID) {
        SECTION.set(None);
        game.open_options_at(false, horizon_client::options_ui::Page::NativeMod(i));
    }
}

/// `Zoomify.tick`, at the end of each client tick.
pub(crate) fn tick(game: &mut crate::game::Game<'_>) {
    // A single player world's server runs a `Game` of its own: the window's only.
    if !game.native_window() {
        return;
    }
    watch(Instant::now());
    if !game.native_on(ID) {
        with_zoom(|z, _| {
            z.zooming = false;
            z.secondary_zooming = false;
            z.clicks.clear();
        });
        return;
    }
    let playing = game.native_playing();
    let mut zoom = ZOOM.with(|z| z.borrow_mut().take());
    if zoom.is_none() {
        with_zoom(|_, _| ());
        zoom = ZOOM.with(|z| z.borrow_mut().take());
    }
    let Some(mut z) = zoom else {
        return;
    };
    let c = cfg();
    // A screen releases every key mapping (KeyMapping.releaseAll).
    if !playing {
        z.held.clear();
    }
    let prev = z.zooming;
    if c.toggle {
        for _ in 0..consume(&mut z, "zoom") {
            z.zooming = !z.zooming;
        }
    } else {
        z.zooming = z.held.contains("zoom");
        consume(&mut z, "zoom");
    }
    for _ in 0..consume(&mut z, "zoom.secondary") {
        z.secondary_zooming = !z.secondary_zooming;
    }
    if c.keybind_scrolling {
        z.scroll_steps += consume(&mut z, "zoom.in") as i64;
        z.scroll_steps -= consume(&mut z, "zoom.out") as i64;
        z.scroll_steps = z.scroll_steps.clamp(z.regular.min_tiers(&c), c.scroll_step_count);
    }
    handle_spyglass(game, &mut z, &c, prev);
    let (zooming, secondary, steps) = (z.zooming, z.secondary_zooming, z.scroll_steps);
    z.regular.tick(&c, zooming, steps);
    z.secondary.tick(&c, secondary, steps);
    z.clicks.clear();
    let display = std::mem::take(&mut z.display_gui);
    ZOOM.with(|cell| *cell.borrow_mut() = Some(z));
    if display {
        open_settings(game);
    }
}

/// `Zoomify.getZoomDivisor`.
fn divisor(partial: f32) -> f64 {
    with_zoom(|z, c| {
        if !z.zooming {
            if !c.retain_zoom_steps {
                z.scroll_steps = 0;
            }
            z.regular.reset();
        }
        let regular = z.regular.divisor(c, f64::from(partial));
        z.previous_divisor = regular;
        regular * z.secondary.divisor(c, f64::from(partial))
    })
}

fn previous_divisor() -> f64 {
    with_zoom(|z, _| z.previous_divisor)
}

/// `CameraMixin.modifyFovWithZoom`: the world's field of view.
pub(crate) fn fov(game: &crate::game::Game<'_>, partial: f32, camera: [f64; 3]) -> ControlFlow<f32> {
    if IN_HOOK.get() || !game.native_on(ID) {
        return ControlFlow::Continue(());
    }
    IN_HOOK.set(true);
    let fov = game.fov_degrees(partial, camera);
    IN_HOOK.set(false);
    ControlFlow::Break((f64::from(fov) / divisor(partial)) as f32)
}

/// `CameraMixin.modifyHudFovWithZoom`: the hand's, with Affect Hand FOV.
pub(crate) fn hud_fov(game: &crate::game::Game<'_>, partial: f32, camera: [f64; 3]) -> ControlFlow<f32> {
    if IN_HOOK.get() || !game.native_on(ID) || !cfg().affect_hand_fov {
        return ControlFlow::Continue(());
    }
    IN_HOOK.set(true);
    let fov = game.hud_fov_degrees(partial, camera);
    IN_HOOK.set(false);
    ControlFlow::Break((f64::from(fov) / divisor(partial)) as f32)
}

/// `CameraMixin.modifyBobbingIntensity`: view bobbing scaled down by the zoom.
pub(crate) fn bob(game: &crate::game::Game<'_>, partial: f32) -> ControlFlow<glam::Mat4> {
    if !game.native_on(ID) || !cfg().relative_view_bobbing {
        return ControlFlow::Continue(());
    }
    let prev = previous_divisor();
    if prev == 1.0 {
        return ControlFlow::Continue(());
    }
    use horizon_client::camera::{BobState, HurtBob, bob_matrix};
    let Some(p) = game.player.as_ref() else {
        return ControlFlow::Break(glam::Mat4::IDENTITY);
    };
    let hurt = &p.health.hurt;
    let bob = game.avatar.interpolated_bob(partial);
    let state = BobState {
        living: Some(HurtBob {
            hurt_time: p.health.hurt_animation_time as f32 - partial,
            hurt_duration: hurt.hurt_duration,
            hurt_dir: p.health.hurt_dir,
            death_time: game.death_time().map(|t| t as f32 + partial),
        }),
        player: Some((
            game.avatar.backwards_interpolated_walk_distance(partial),
            (f64::from(bob) / (1.0 + 0.2 * (prev - 1.0))) as f32,
        )),
    };
    let tilt = game.settings.file.double("damageTiltStrength");
    ControlFlow::Break(
        glam::Mat4::from_cols_array(&bob_matrix(&state, tilt, game.settings.view_bobbing).m)
            * game.native_shake_matrix(partial),
    )
}

/// `MouseMixin`: relative sensitivity, the cinematic camera while zooming, and
/// the spyglass's slower turning only when combining with it.
pub(crate) fn look(game: &crate::game::Game<'_>) -> ControlFlow<horizon_client_input::mouse::LookOptions> {
    if IN_HOOK.get() || !game.native_on(ID) {
        return ControlFlow::Continue(());
    }
    IN_HOOK.set(true);
    let mut o = game.look_options();
    IN_HOOK.set(false);
    let c = cfg();
    let (zooming, secondary, prev) = with_zoom(|z, _| (z.zooming, z.secondary_zooming, z.previous_divisor));
    o.sensitivity /= 1.0 + c.relative_sensitivity as f64 / 100.0 * (prev - 1.0);
    o.smooth_camera = o.smooth_camera || secondary || (zooming && c.cinematic_camera > 0);
    if c.spyglass != 0 {
        o.scoping = false;
    }
    ControlFlow::Break(o)
}

/// `MouseMixin.scrollStepCounter`: the wheel zooms while zooming.
pub(crate) fn scroll(game: &crate::game::Game<'_>, accumulated: [i32; 2], canceled: &mut bool) {
    if !game.native_on(ID) {
        return;
    }
    let delta = i64::from(accumulated[1]) + i64::from(accumulated[0]);
    let c = cfg();
    with_zoom(|z, c2| {
        if c.scroll_zoom && z.zooming && delta != 0 && !c.keybind_scrolling {
            if delta > 0 {
                z.scroll_steps += 1;
            } else {
                z.scroll_steps -= 1;
            }
            z.scroll_steps = z.scroll_steps.clamp(z.regular.min_tiers(c2), c2.scroll_step_count);
            *canceled = true;
        }
    });
}

/// `HudMixin.preventHudRender`: no HUD during the secondary zoom.
pub(crate) fn hud_layer(game: &crate::game::Game<'_>, canceled: &mut bool) {
    if game.native_on(ID) && cfg().secondary_hide_hud && with_zoom(|z, _| z.secondary_zooming) {
        *canceled = true;
    }
}

/// The spyglass's field of view: 1 instead of a tenth unless combining.
pub(crate) fn fov_modifier(game: &crate::game::Game<'_>, player: &horizon_client::camera::FovPlayer, modifier: &mut f32) {
    use horizon_client::camera::UseItem;
    if !game.native_on(ID) || cfg().spyglass == 0 {
        return;
    }
    let scoping = matches!(player.using, Some((UseItem::Spyglass, _)));
    if scoping && game.camera_type.first_person() {
        *modifier = 1.0;
    }
}

/// `Hud.extractCameraOverlays` with Zoomify's overlay visibility.
pub(crate) fn spyglass_overlay(game: &mut crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) -> ControlFlow<()> {
    if !game.native_on(ID) {
        return ControlFlow::Continue(());
    }
    game.scope_scale += 0.5 * game.frame_ticks * (1.125 - game.scope_scale);
    if !game.camera_type.first_person() {
        return ControlFlow::Break(());
    }
    let c = cfg();
    let zooming = with_zoom(|z, _| z.zooming);
    let scoping = game.scoping();
    if !should_render_overlay(game, &c, zooming, scoping) {
        game.scope_scale = 0.5;
        return ControlFlow::Break(());
    }
    let draws = horizon_client::hud_overlays::spyglass(batch.width as i32, batch.height as i32, game.scope_scale);
    horizon_client::hud_layout::paint(batch, &game.gui.atlas, &draws);
    ControlFlow::Break(())
}

/// `SpyglassItemMixin`: the spyglass's own sounds.
fn spyglass_sounds() -> bool {
    let c = cfg();
    !(c.sound == 0 || (c.sound == 3 && c.overlay == 0))
}

pub(crate) fn use_spyglass(game: &mut crate::game::Game<'_>) -> ControlFlow<()> {
    if !game.native_on(ID) || spyglass_sounds() {
        return ControlFlow::Continue(());
    }
    game.award_stat("used", "minecraft:spyglass", 1);
    game.start_using_item();
    ControlFlow::Break(())
}

pub(crate) fn stop_spyglass(game: &crate::game::Game<'_>) -> ControlFlow<()> {
    if !game.native_on(ID) || spyglass_sounds() {
        return ControlFlow::Continue(());
    }
    ControlFlow::Break(())
}

/// `/zoomify`: the settings, after this command (displayGui).
pub(crate) fn command(game: &mut crate::game::Game<'_>, command: &str) -> bool {
    if command.trim() != "zoomify" || !game.native_on(ID) {
        return false;
    }
    with_zoom(|z, _| z.display_gui = true);
    true
}

// ---------------------------------------------------------------------------------------------
// The settings (YACL's screen as the mod's settings page)
// ---------------------------------------------------------------------------------------------

fn shown(e: &Entry, v: &str) -> String {
    match e.kind {
        Kind::Bool => (if v == "true" { "ON" } else { "OFF" }).to_owned(),
        Kind::Enum(names) => names.iter().find(|(n, _)| *n == v).map_or(v.to_owned(), |(_, d)| (*d).to_owned()),
        Kind::Int(_, _, _, f) | Kind::Double(_, _, _, f) => {
            let x = v.parse::<f64>().unwrap_or(0.0);
            match f {
                Fmt::Plain => v.to_owned(),
                Fmt::Times => format!("{}x", x as i64),
                Fmt::TimesTenth => format!("{:.1}x", x / 100.0),
                Fmt::Secs => format!("{x:.1} secs"),
                Fmt::SecsInstant if x == 0.0 => "Instant".to_owned(),
                Fmt::SecsInstant => format!("{x:.2} secs"),
                Fmt::PercentInstant if x == 0.0 => "Instant".to_owned(),
                Fmt::PercentInstant => format!("{}%", x as i64),
                Fmt::PercentOff if x == 0.0 => "OFF".to_owned(),
                Fmt::PercentOff => format!("{}%", x as i64),
            }
        }
    }
}

fn section_entries(s: usize) -> impl Iterator<Item = &'static Entry> {
    ENTRIES.iter().filter(move |e| e.section == s)
}

const PRESETS: [&str; 3] = ["Default", "OptiFine", "OK Zoomer"];

pub(crate) fn settings(buttons: &mut Vec<String>) {
    match SECTION.get() {
        None => buttons.extend(SECTIONS.iter().map(|s| format!("{s}..."))),
        Some(3) => {
            buttons.extend(PRESETS.iter().map(|p| format!("{p}: Apply Preset")));
            buttons.push("Done".to_owned());
        }
        Some(s) => {
            for e in section_entries(s) {
                buttons.push(format!("{}: {}", e.caption, shown(e, &get(e.key))));
            }
            buttons.extend(["Undo".to_owned(), "Reset".to_owned(), "Done".to_owned()]);
        }
    }
}

fn step(e: &Entry, back: bool, many: bool) -> String {
    let v = get(e.key);
    let n = if many { 10.0 } else { 1.0 };
    match e.kind {
        Kind::Bool => (if v == "true" { "false" } else { "true" }).to_owned(),
        Kind::Enum(names) => {
            let i = names.iter().position(|(x, _)| *x == v).unwrap_or(0);
            let len = names.len();
            names[if back { (i + len - 1) % len } else { (i + 1) % len }].0.to_owned()
        }
        Kind::Int(min, max, step, _) => {
            let x = v.parse::<i64>().unwrap_or(min);
            let d = step * n as i64;
            let next = if back { x - d } else { x + d };
            // Past an end, the slider starts again from the other one.
            let next = if next > max { min } else if next < min { max } else { next };
            next.to_string()
        }
        Kind::Double(min, max, step, _) => {
            let x = v.parse::<f64>().unwrap_or(min);
            let d = step * n;
            let raw = if back { x - d } else { x + d };
            let snapped = ((raw - min) / step).round() * step + min;
            let next = if snapped > max + 1e-9 { min } else if snapped < min - 1e-9 { max } else { snapped };
            fmt_double((next * 1000.0).round() / 1000.0)
        }
    }
}

/// `Presets.apply`.
fn apply_preset(game: &mut crate::game::Game<'_>, i: usize) {
    let mut map = defaults();
    map.insert("_firstLaunch", "false".into());
    let changes: &[(&str, &str)] = match i {
        1 => &[
            ("zoomInTransition", "instant"),
            ("zoomOutTransition", "instant"),
            ("scrollZoom", "false"),
            ("relativeSensitivity", "0"),
            ("relativeViewBobbing", "false"),
            ("cinematicCamera", "100"),
        ],
        2 => &[
            ("zoomInTime", "0.25"),
            ("zoomOutTime", "0.25"),
            ("relativeSensitivity", "50"),
            ("relativeViewBobbing", "false"),
            ("scrollZoomSmoothness", "25"),
        ],
        _ => &[],
    };
    for (k, v) in changes {
        if let Some(e) = ENTRIES.iter().find(|e| e.key == *k) {
            map.insert(e.key, (*v).to_owned());
        }
    }
    with_values(|v| {
        v.map = map;
        write_file(&v.map);
        v.stamp = stamp();
    });
    let title = "Preset Applied".to_owned();
    let message = format!("Zoomify applied the {} preset.", PRESETS[i]);
    match &mut game.server_packs.load_fail {
        Some(toast) => toast.reset(title, Some(message)),
        None => {
            if game.toast_allowed("system", "periodic_notification") {
                game.server_packs.load_fail = Some(horizon_client::toasts::SystemToast::new(title, Some(message)));
            }
        }
    }
}

pub(crate) fn click(game: &mut crate::game::Game<'_>, button: usize) {
    let back = game.modifiers.shift_key();
    let many = game.modifiers.control_key();
    let Some(s) = SECTION.get() else {
        if button < SECTIONS.len() {
            SECTION.set(Some(button));
            UNDO.with(|u| *u.borrow_mut() = section_entries(button).map(|e| (e.key, get(e.key))).collect());
        }
        return;
    };
    if s == 3 {
        if button < PRESETS.len() {
            apply_preset(game, button);
        } else {
            SECTION.set(None);
        }
        return;
    }
    let entries: Vec<&Entry> = section_entries(s).collect();
    match button.checked_sub(entries.len()) {
        None => {
            let e = entries[button];
            if NEEDS_SCROLL_ZOOM.contains(&e.key) && get("scrollZoom") != "true" {
                return;
            }
            set(e.key, step(e, back, many));
        }
        Some(0) => {
            for (k, v) in UNDO.with(|u| u.borrow().clone()) {
                set(k, v);
            }
        }
        Some(1) => {
            for e in &entries {
                set(e.key, e.default.to_owned());
            }
        }
        _ => SECTION.set(None),
    }
}

/// Leaving the settings page goes back to its category list.
pub(crate) fn frame(game: &crate::game::Game<'_>) {
    use horizon_client::options_ui::Page;
    let on_page = game.screen == crate::game::Screen::Options
        && matches!(game.ui.options.page(), Page::NativeMod(i)
            if game.settings.mods.installed.get(i).is_some_and(|m| m.id == ID));
    if !on_page {
        SECTION.set(None);
    }
}
