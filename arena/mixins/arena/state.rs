// Arena HUD — 16.10.1 (mihehk): the mod's state and its reach into the
// game. Each frame (the HUD hook) reads the keys, counts clicks and frames,
// turns the camera into Mousestrokes, builds a `Snapshot` and draws the
// modules where they are placed; attacks (sent) and damage events
// (received) feed Reach and Combo; the world hook draws Hitbox and Block
// Overlay; Right Alt opens the HUD editor (a mod screen).
//
// Mounted in `game::remote` (horizon-mixin.toml) so it can name the play
// packets `S` / `C` as BetterHitreg does. What every Horizon call here is
// based on: `native_mapping`, `S::Attack(a).id`, `C::DamageEvent(p).id`,
// `game.mobs` (`id`, `position`, `alive`, `kind.id`, `extra.main_hand`,
// `has_effect`), `game.remote.entity_id`, `game.player` (`position`,
// `dimensions.eye_height`, `sprinting`), and for Target HUD a mob's
// `health`, `absorption`, `custom_name_text`, `finalize::max_health`,
// `armor::values` and `remote.level_player_names`: BetterHitreg,
// HealthIndicators, HatLayers and OzgurKamera use them. The few guesses are
// marked UNVERIFIED below.
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Instant;

use super::arena_core::{self as core, Module, Options, Snapshot};
use super::arena_editor as ed;

pub const ID: &str = "arena";
pub const NAME: &str = "Arena HUD";
pub const VERSION: &str = "16.10.1";
pub const DESCRIPTION: &str =
    "PvP HUD: Keystrokes, CPS, Mousestrokes, Reach, Combo, Hitbox, Toggle Sprint, Armor Status, Potion Effects, Ping, FPS, Block Overlay, Target HUD, Speed, Coordinates, Session Stats. Sağ Alt: HUD editörü · /arena";
pub const PREFIX: &str = "[Arena] ";

/// Mod keys: name, label, default key.
pub const KEYS: [(&str, &str, &str); 1] = [("editor", "Arena HUD: HUD editörü", "key.keyboard.right.alt")];

/// The mappings Keystrokes shows, in `Snapshot::keys` order.
const MAPPINGS: [&str; 9] = ["key.forward", "key.left", "key.back", "key.right", "key.jump", "key.sneak", "key.sprint", "key.attack", "key.use"];

/// The effects Potion Effects looks for (vanilla's, by id).
const EFFECTS: [&str; 33] = [
    "speed", "slowness", "haste", "mining_fatigue", "strength", "jump_boost", "nausea", "regeneration", "resistance",
    "fire_resistance", "water_breathing", "invisibility", "blindness", "night_vision", "hunger", "weakness", "poison",
    "wither", "health_boost", "absorption", "saturation", "glowing", "levitation", "luck", "unluck", "slow_falling",
    "conduit_power", "dolphins_grace", "bad_omen", "hero_of_the_village", "darkness", "trial_omen", "raid_omen",
];

#[derive(Default)]
pub struct State {
    clicks: [core::Clicks; 2],
    frames: core::Frames,
    pub fight: core::Fight,
    fade: core::Fade,
    mouse: core::Mouse,
    speed: core::Speed,
    /// The mappings' state last frame (for presses).
    held: [bool; 9],
    /// Toggle Sprint's toggle.
    sprint_on: bool,
    /// What the last frame showed, for the editor.
    pub last: Snapshot,
    /// Where each HUD module was drawn last frame.
    pub boxes: Vec<(Module, [f32; 4])>,
    pub editor: ed::Editor,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

pub struct Guard(MutexGuard<'static, Option<State>>);

impl std::ops::Deref for Guard {
    type Target = State;
    fn deref(&self) -> &State {
        self.0.as_ref().expect("made in state()")
    }
}

impl std::ops::DerefMut for Guard {
    fn deref_mut(&mut self) -> &mut State {
        self.0.as_mut().expect("made in state()")
    }
}

pub fn state() -> Guard {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    g.get_or_insert_with(State::default);
    Guard(g)
}

/// Milliseconds since the mod first counted.
pub fn now() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// A module's options as the game stores them.
pub fn options<'a>(m: Module, get: &'a dyn Fn(&str) -> Option<String>) -> Options<'a> {
    Options { module: m, get }
}

fn on(get: &dyn Fn(&str) -> Option<String>, m: Module) -> bool {
    options(m, get).on("enabled")
}

/// A module's stored place, or where it starts.
fn place(get: &dyn Fn(&str) -> Option<String>, m: Module) -> [f32; 2] {
    let d = m.default_place();
    let read = |k: &str, d: f32| get(&Options::key(m, k)).and_then(|v| v.parse().ok()).unwrap_or(d);
    [read("x", d[0]), read("y", d[1])]
}

/// Every enabled HUD module laid out on a `screen`: its box and draws.
pub fn layout(
    get: &dyn Fn(&str) -> Option<String>,
    snap: &Snapshot,
    screen: [f32; 2],
    fade: &mut dyn FnMut(&'static str, bool) -> f32,
    measure: &dyn Fn(&str) -> f32,
) -> Vec<(Module, [f32; 4], Vec<core::Draw>)> {
    let mut out = Vec::new();
    for m in core::ALL {
        if !m.on_hud() || !on(get, m) {
            continue;
        }
        let o = options(m, get);
        let Some((_, size)) = core::draw(&o, snap, [0.0, 0.0], fade, measure) else { continue };
        if size[0] <= 0.0 || size[1] <= 0.0 {
            continue;
        }
        let at = ed::at(place(get, m), size, screen);
        let Some((draws, _)) = core::draw(&o, snap, at, fade, measure) else { continue };
        out.push((m, [at[0], at[1], size[0], size[1]], draws));
    }
    out
}

fn paint(game: &super::Game<'_>, batch: &mut horizon_client::hud::HudBatch, draws: &[core::Draw]) {
    for d in draws {
        match d {
            core::Draw::Fill(rect, argb) => {
                game.native_fill(batch, *rect, *argb);
            }
            core::Draw::Text(text, at, rgb, shadow) => {
                game.native_text(batch, text, *at, *rgb, *shadow);
            }
        }
    }
}

/// Each frame: read the keys and the camera, count, draw the modules.
pub(crate) fn hud(game: &mut super::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    if !game.native_on(ID) {
        return;
    }
    let t = now();
    let keys: [bool; 9] = MAPPINGS.map(|m| game.native_mapping(m).is_some_and(|s| s.0));
    let look = game.native_player().map(|p| (p.yaw, p.pitch));
    let dimension = game.native_player().map(|p| p.dimension.to_string()).unwrap_or_default();
    let game = &*game;
    let get = |k: &str| game.native_setting(ID, k);
    let (ping, armor, hand, effects, sprinting) = (horizon_ping(game), horizon_armor(game), horizon_hand(game), horizon_effects(game), horizon_sprinting(game));
    let feet = game.player.as_ref().map(|p| [p.position.x, p.position.y, p.position.z]);
    let eye = game.player.as_ref().map(|p| [p.position.x, p.position.y + f64::from(p.dimensions.eye_height), p.position.z]);
    let screen = [batch.width, batch.height];
    let measure = |s: &str| game.native_text_width(s);
    let laid = {
        let mut s = state();
        let s = &mut *s;
        // A press is a mapping going down since the last frame.
        for (i, c) in [(7, 0), (8, 1)] {
            if keys[i] && !s.held[i] {
                s.clicks[c].press(t);
            }
        }
        if keys[6] && !s.held[6] && options(Module::ToggleSprint, &get).on("enabled") && options(Module::ToggleSprint, &get).on("sprint") {
            s.sprint_on = !s.sprint_on;
        }
        s.held = keys;
        if let Some((yaw, pitch)) = look {
            s.mouse.look(yaw, pitch);
        }
        let fps = s.frames.frame(t);
        let reach_timeout = options(Module::Reach, &get).number("timeout") as u64;
        let combo_timeout = options(Module::Combo, &get).number("timeout") as u64;
        let target_timeout = options(Module::TargetHud, &get).number("timeout") as u64;
        let target = s.fight.target.filter(|(_, at)| t.saturating_sub(*at) <= target_timeout).and_then(|(id, _)| target_of(game, id, eye));
        let vertical = options(Module::Speed, &get).on("vertical");
        let speed = feet.map_or(0.0, |p| s.speed.at(t, p, vertical));
        let snap = Snapshot {
            keys,
            cps: [s.clicks[0].per_second(t), s.clicks[1].per_second(t)],
            mouse: s.mouse.dot,
            reach: s.fight.last_reach.filter(|(_, at)| t.saturating_sub(*at) <= reach_timeout).map(|r| r.0),
            combo: s.fight.combo(t, combo_timeout),
            ping,
            fps,
            armor,
            hand,
            effects,
            sprint: (s.sprint_on, sprinting),
            target,
            speed,
            pos: feet.zip(look).map(|(p, (yaw, _))| (p, yaw, dimension)),
            stats: core::Stats { hits: s.fight.hits, best: s.fight.best, average: s.fight.average_reach() },
        };
        let fade_ms = options(Module::Keystrokes, &get).number("fade");
        let fade = &mut s.fade;
        let mut level = |k: &'static str, down: bool| fade.level(k, down, t, fade_ms);
        let laid = layout(&get, &snap, screen, &mut level, &measure);
        s.boxes = laid.iter().map(|(m, r, _)| (*m, *r)).collect();
        s.last = snap;
        laid
    };
    // The editor screen draws the modules itself.
    if game.native_mod_screen() == Some(ID) {
        return;
    }
    for (_, _, draws) in laid {
        paint(game, batch, &draws);
    }
}

/// An attack sent: its reach, measured now from the eyes to the target's box.
pub(crate) fn on_send(game: &super::Game<'_>, packet: &super::S) {
    let super::S::Attack(attack) = packet else {
        return;
    };
    if !game.native_on(ID) {
        return;
    }
    let target = attack.id as u32;
    let Some(mob) = game.mobs.iter().find(|m| m.id == target) else {
        return;
    };
    let Some(p) = game.player.as_ref() else {
        return;
    };
    let eye = [p.position.x, p.position.y + f64::from(p.dimensions.eye_height), p.position.z];
    let (lo, hi) = mob_box(mob);
    state().fight.attack(i64::from(target), core::reach(eye, lo, hi), now());
}

/// A damage event: a hit of ours (Reach, Combo), or us hit (the combo ends).
pub(crate) fn on_packet(game: &super::Game<'_>, packet: &super::C) {
    if let super::C::DamageEvent(p) = packet {
        let me = game.remote.as_ref().map(|r| r.entity_id as i64);
        state().fight.damaged(p.id as i64, me, now());
    }
}

/// A mob's hit box. UNVERIFIED for anything but players: Horizon's mob
/// sizes are not known to us, so every mob gets a player's 0.6 x 1.8.
fn mob_box(mob: &horizon_client::mobs::Mob) -> ([f64; 3], [f64; 3]) {
    let (w, h) = (0.6, 1.8);
    let b = [mob.position.x, mob.position.y, mob.position.z];
    ([b[0] - w / 2.0, b[1], b[2] - w / 2.0], [b[0] + w / 2.0, b[1] + h, b[2] + w / 2.0])
}

/// The entity last hit, for Target HUD, while it is alive and tracked. Its
/// name, health, max health, absorption and armour are read the way
/// HealthIndicators reads them.
fn target_of(game: &super::Game<'_>, id: i64, eye: Option<[f64; 3]>) -> Option<core::Target> {
    let m = game.mobs.iter().find(|m| m.id as i64 == id && m.alive())?;
    let name = m
        .custom_name_text
        .as_ref()
        .map(|t| t.plain())
        .or_else(|| game.remote.as_ref().and_then(|r| r.level_player_names.get(&i32::try_from(m.id).ok()?).cloned()))
        .unwrap_or_else(|| core::item_name(&m.kind.id));
    let (lo, hi) = mob_box(m);
    Some(core::Target {
        name,
        health: m.health,
        max: horizon_client::mobs::finalize::max_health(m),
        absorption: m.absorption,
        armor: horizon_client::mobs::armor::values(m).0 as f32,
        hand: m.extra.main_hand.as_deref().map(|h| h.to_owned()),
        distance: eye.map_or(0.0, |e| core::reach(e, lo, hi)),
    })
}

/// What the world hook draws: (min, max, ARGB, line width; 0: a fill).
pub(crate) fn world_boxes(game: &super::Game<'_>) -> Vec<([f64; 3], [f64; 3], u32, f32)> {
    if !game.native_on(ID) {
        return Vec::new();
    }
    let get = |k: &str| game.native_setting(ID, k);
    let mut out = Vec::new();
    let hb = options(Module::Hitbox, &get);
    if hb.on("enabled") {
        let me = game.remote.as_ref().map(|r| r.entity_id as i64);
        let from = game.player.as_ref().map(|p| [p.position.x, p.position.y, p.position.z]);
        let range = hb.number("range");
        let players_only = hb.on("players_only");
        for m in game.mobs.iter() {
            if Some(m.id as i64) == me || !m.alive() || (players_only && &*m.kind.id != "minecraft:player") {
                continue;
            }
            if let Some(f) = from {
                let d = [m.position.x - f[0], m.position.y - f[1], m.position.z - f[2]];
                if (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() > range {
                    continue;
                }
            }
            let (lo, hi) = mob_box(m);
            out.push((lo, hi, hb.color("color"), hb.number("width") as f32));
            if hb.on("eye_line") {
                let y = lo[1] + 1.62;
                out.push(([lo[0], y - 0.01, lo[2]], [hi[0], y + 0.01, hi[2]], 0xFFFF_0000, 0.0));
            }
        }
    }
    let bo = options(Module::BlockOverlay, &get);
    if bo.on("enabled") {
        if let Some(h) = game.native_raycast(5.0) {
            let lo = [f64::from(h.block_x) - 0.002, f64::from(h.block_y) - 0.002, f64::from(h.block_z) - 0.002];
            let hi = lo.map(|v| v + 1.004);
            if bo.on("fill") {
                out.push((lo, hi, bo.color("fill_color"), 0.0));
            }
            if bo.on("outline") {
                out.push((lo, hi, bo.color("outline_color"), bo.number("width") as f32));
            }
        }
    }
    out
}

/// Each tick: Toggle Sprint keeps the player sprinting while it is on and
/// they walk forward, not sneaking.
pub(crate) fn tick(game: &mut super::Game<'_>) {
    if !game.native_on(ID) {
        return;
    }
    let on = state().sprint_on;
    if !on {
        return;
    }
    let forward = game.native_mapping("key.forward").is_some_and(|s| s.0);
    let sneak = game.native_mapping("key.sneak").is_some_and(|s| s.0);
    if forward && !sneak {
        horizon_set_sprinting(game);
    }
}

// ---- Horizon guesses (UNVERIFIED: Horizon's source is private) ----------
// If the first build fails, these are the lines to fix; each module that
// needs one shows "?" or nothing until it works.

/// The player's ping as the tab list has it.
fn horizon_ping(game: &super::Game<'_>) -> Option<u32> {
    let r = game.remote.as_ref()?;
    // `r.profile` is our UUID and `tab_players` is keyed by UUID (both as
    // HatLayers uses them); `latency` is the guess.
    let me = r.tab_players.get(&r.profile)?;
    Some(me.latency.max(0) as u32)
}

/// The player as the mob list has them (their equipment and effects).
fn own_mob<'g>(game: &'g super::Game<'_>) -> Option<&'g horizon_client::mobs::Mob> {
    let me = game.remote.as_ref()?.entity_id as i64;
    game.mobs.iter().find(|m| m.id as i64 == me)
}

/// The armour worn, head first (durability is not known: None).
fn horizon_armor(game: &super::Game<'_>) -> Vec<(String, Option<f32>)> {
    let Some(m) = own_mob(game) else { return Vec::new() };
    [&m.extra.head, &m.extra.chest, &m.extra.legs, &m.extra.feet]
        .into_iter()
        .filter_map(|slot| slot.as_deref().map(|id| (id.to_owned(), None)))
        .collect()
}

/// The main hand's item (`extra.main_hand` is BetterHitreg's).
fn horizon_hand(game: &super::Game<'_>) -> Option<(String, Option<f32>)> {
    own_mob(game)?.extra.main_hand.as_deref().map(|id| (id.to_owned(), None))
}

/// Active effects (`has_effect` is HealthIndicators'; time left is not known).
fn horizon_effects(game: &super::Game<'_>) -> Vec<(String, Option<u32>)> {
    let Some(m) = own_mob(game) else { return Vec::new() };
    EFFECTS
        .iter()
        .filter(|e| m.has_effect(&format!("minecraft:{e}")))
        .map(|e| (core::item_name(e), None))
        .collect()
}

/// Sprinting now (`sprinting` is read by BetterHitreg).
fn horizon_sprinting(game: &super::Game<'_>) -> bool {
    game.player.as_ref().is_some_and(|p| p.sprinting)
}

/// Starts sprinting, as holding the sprint key would.
fn horizon_set_sprinting(game: &mut super::Game<'_>) {
    if let Some(p) = game.player.as_mut() {
        p.sprinting = true;
    }
}

// ---- the editor screen and `/arena` ------------------------------------

/// The editor screen: the modules where they are, and the editor over them.
pub(crate) fn screen(game: &super::Game<'_>, batch: &mut horizon_client::hud::HudBatch, mouse: [f32; 2]) {
    let get = |k: &str| game.native_setting(ID, k);
    let measure = |s: &str| game.native_text_width(s);
    let screen = [batch.width, batch.height];
    let (laid, over) = {
        let mut s = state();
        let s = &mut *s;
        s.editor.size = screen;
        let snap = s.last.clone();
        let mut still = |_: &'static str, _: bool| 0.0;
        let laid = layout(&get, &snap, screen, &mut still, &measure);
        s.boxes = laid.iter().map(|(m, r, _)| (*m, *r)).collect();
        let enabled = |m: Module| on(&get, m);
        let over = s.editor.draw(&s.boxes, &enabled, mouse);
        (laid, over)
    };
    for (_, _, draws) in laid {
        paint(game, batch, &draws);
    }
    paint(game, batch, &over);
}

fn store(game: &super::Game<'_>, m: Module, option: &str, value: &str) {
    game.native_set_setting(ID, &Options::key(m, option), value);
}

/// A button down on the editor.
pub(crate) fn press(game: &super::Game<'_>, at: [f32; 2], button: i32) {
    let click = {
        let mut s = state();
        let boxes = s.boxes.clone();
        s.editor.press(at, button, &boxes)
    };
    let get = |k: &str| game.native_setting(ID, k);
    match click {
        ed::Click::Toggle(m) => store(game, m, "enabled", if on(&get, m) { "false" } else { "true" }),
        ed::Click::Reset(m) => {
            let d = m.default_place();
            store(game, m, "x", &d[0].to_string());
            store(game, m, "y", &d[1].to_string());
        }
        ed::Click::Grab(_) | ed::Click::Nothing => {}
    }
}

/// The pointer dragged on the editor: the held module follows.
pub(crate) fn drag(game: &super::Game<'_>, at: [f32; 2]) {
    let moved = {
        let mut s = state();
        let boxes = s.boxes.clone();
        s.editor.drag(at, &boxes)
    };
    if let Some((m, f)) = moved {
        store(game, m, "x", &format!("{:.4}", f[0]));
        store(game, m, "y", &format!("{:.4}", f[1]));
    }
}

/// `/arena …`: the chat lines to show.
pub(crate) fn command(game: &super::Game<'_>, args: &[&str]) -> Vec<String> {
    let get = |k: &str| game.native_setting(ID, k);
    let p = PREFIX;
    let Some(first) = args.first() else {
        let mut out = vec![format!("{p}{NAME} {VERSION} — modüller (/arena <modül> aç/kapat, /arena <modül> <ayar> <değer>):")];
        let list: Vec<String> = core::ALL.iter().map(|m| format!("{} {}", if on(&get, *m) { "■" } else { "□" }, m.id())).collect();
        out.push(list.join(" · "));
        out.push("Sağ Alt: HUD editörü (sürükle, sağ tık: ilk yerine).".into());
        return out;
    };
    let Some(m) = Module::parse(first) else {
        return vec![format!("{p}Bilinmeyen modül: {first}. /arena listeyi gösterir.")];
    };
    match (args.get(1), args.get(2)) {
        (None, _) => {
            let now_on = !on(&get, m);
            store(game, m, "enabled", if now_on { "true" } else { "false" });
            vec![format!("{p}{} {}.", m.name(), if now_on { "açık" } else { "kapalı" })]
        }
        (Some(&"ayarlar" | &"settings"), _) => {
            let o = options(m, &get);
            let mut out = vec![format!("{p}{} ayarları:", m.name())];
            for (k, _, what) in core::COMMON.iter().chain(m.options()) {
                if !m.on_hud() && matches!(*k, "background" | "bg_color" | "text_color" | "brackets") {
                    continue;
                }
                out.push(format!("{k} = {} — {what}", o.text(k)));
            }
            out
        }
        (Some(&"temizle" | &"clear"), _) if m == Module::Stats => {
            state().fight.clear_stats();
            vec![format!("{p}Session Stats sıfırlandı.")]
        }
        (Some(&"sifirla" | &"sıfırla" | &"reset"), _) => {
            let d = m.default_place();
            store(game, m, "x", &d[0].to_string());
            store(game, m, "y", &d[1].to_string());
            vec![format!("{p}{} ilk yerine döndü.", m.name())]
        }
        (Some(option), Some(value)) => match core::checked(m, option, value) {
            Ok(v) => {
                store(game, m, option, &v);
                vec![format!("{p}{} · {option} = {v}", m.name())]
            }
            Err(e) => vec![format!("{p}{e}")],
        },
        (Some(option), None) => vec![format!("{p}{option} için bir değer yaz, ör. /arena {} {option} <değer> (/arena {} ayarlar)", m.id(), m.id())],
    }
}
