// Pusula — 16.10.1 (mihehk): the mod's state and its reach into the game.
// Each frame scans a few loaded chunks around the player (nearest first),
// keeps them per world and dimension, saves the changed regions every 30
// seconds, and notices a death. The HUD hook draws the minimap and the
// waypoints in the world; M opens the world map (the mod's own screen).
//
// Mounted in `game::remote` (horizon-mixin.toml), as Arena HUD, so it can read
// `session.address`. What every Horizon call here is based on:
// - blocks: `game.world.block`, `game.world.chunks`, `res.facts().is_air` and
//   `state_string` (BetterHitreg, HatLayers, Eksen);
// - `game.player.position`, `native_player().{yaw, dimension}`, `game.mobs`
//   (`id`, `position`, `alive`, `kind.id`), `remote.entity_id`,
//   `remote.level_player_names`, `has_singleplayer_server`,
//   `locator_camera` (Arena HUD, VoiceChat, Eksen, HealthIndicators);
// - `native_data_dir`, `native_send_chat`, the screen and key hooks (VoiceChat,
//   Eksen). The guesses are marked UNVERIFIED, at the end.
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Instant;

use super::pusula_core::{self as core, Cell, Look, Map, Settings, Waypoint};

pub const ID: &str = "pusula";
pub const NAME: &str = "Pusula";
pub const VERSION: &str = "16.10.1";
pub const DESCRIPTION: &str =
    "Minimap ve dünya haritası: gezdiğin yerler kaydedilir; waypointler, radar, mağara modu, ölüm noktası. M: dünya haritası · B: yeni waypoint · = / -: yakınlaştır · /pusula";
pub const PREFIX: &str = "[Pusula] ";

/// Mod keys: name, label, default key.
pub const KEYS: [(&str, &str, &str); 4] = [
    ("map", "Pusula: Dünya haritası", "key.keyboard.m"),
    ("waypoint", "Pusula: Yeni waypoint", "key.keyboard.b"),
    ("zoom_in", "Pusula: Minimap yakınlaştır", "key.keyboard.equal"),
    ("zoom_out", "Pusula: Minimap uzaklaştır", "key.keyboard.minus"),
];

/// Chunks scanned around the player, each way.
const RADIUS: i32 = 12;
/// How often a chunk is scanned again: next to the player, and further.
const NEAR_MS: u64 = 1500;
const FAR_MS: u64 = 20_000;
const SAVE_MS: u64 = 30_000;

/// Every chunk offset within `RADIUS`, nearest first.
fn offsets() -> &'static [(i32, i32)] {
    static O: OnceLock<Vec<(i32, i32)>> = OnceLock::new();
    O.get_or_init(|| {
        let mut v: Vec<(i32, i32)> = (-RADIUS..=RADIUS).flat_map(|x| (-RADIUS..=RADIUS).map(move |z| (x, z))).filter(|(x, z)| x * x + z * z <= RADIUS * RADIUS).collect();
        v.sort_by_key(|(x, z)| x * x + z * z);
        v
    })
}

#[derive(Default)]
pub struct State {
    surface: Map,
    /// Under a roof: the floor around the player's feet, not saved.
    cave: Map,
    cave_y: Option<i32>,
    in_cave: bool,
    /// When each chunk was last scanned (x, z, cave).
    scanned: HashMap<(i32, i32, bool), u64>,
    /// What each block state looks like, by state id.
    looks: HashMap<u32, Look>,
    /// The world's folder and the dimension the maps are of.
    world: Option<(PathBuf, String)>,
    /// Regions read from disk (or found missing).
    loaded: HashSet<(i32, i32)>,
    pub waypoints: Vec<Waypoint>,
    waypoints_dirty: bool,
    saved_at: u64,
    alive: Option<bool>,
    last_feet: Option<[f64; 3]>,
    /// `/pusula dunya <ad>`: one server, several worlds.
    sub: Option<String>,
    pub screen: core::Screen,
    /// Chat commands the world map asked for, sent next frame.
    commands: Vec<String>,
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

pub fn now() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

fn dimension(game: &super::Game<'_>) -> Option<String> {
    let d = game.native_player()?.dimension;
    Some(if d.contains(':') { d.to_owned() } else { format!("minecraft:{d}") })
}

/// The world the player is in: its folder (server address, or single player,
/// and the `/pusula dunya` name) and the dimension.
fn world_of(game: &super::Game<'_>, sub: &Option<String>) -> Option<(PathBuf, String)> {
    let dim = dimension(game)?;
    let base = if game.has_singleplayer_server() { "tek-oyunculu".to_owned() } else { game.remote.as_ref()?.session.address.clone() };
    let name = match sub {
        Some(s) => format!("{base}-{s}"),
        None => base,
    };
    Some((game.native_data_dir(ID).join(core::folder(&name)), dim))
}

impl State {
    fn region_dir(&self) -> Option<PathBuf> {
        let (dir, dim) = self.world.as_ref()?;
        Some(dir.join(core::folder(dim)))
    }

    /// Reads a region from disk once, before anything in it is scanned (so a
    /// save never writes a region without what was already on disk).
    fn ensure(&mut self, rx: i32, rz: i32) {
        if !self.loaded.insert((rx, rz)) {
            return;
        }
        let Some(dir) = self.region_dir() else { return };
        if let Ok(bytes) = std::fs::read(dir.join(format!("{rx}_{rz}.psl"))) {
            self.surface.load(&bytes);
        }
    }

    /// Every changed region and the waypoints, written now.
    fn save(&mut self) -> Result<(), String> {
        let Some(dir) = self.region_dir() else { return Ok(()) };
        let dirty: Vec<(i32, i32)> = self.surface.dirty.drain().collect();
        if !dirty.is_empty() {
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        }
        for (rx, rz) in dirty {
            write(&dir.join(format!("{rx}_{rz}.psl")), &self.surface.region(rx, rz))?;
        }
        if self.waypoints_dirty {
            if let Some((world, _)) = &self.world {
                std::fs::create_dir_all(world).map_err(|e| e.to_string())?;
                write(&world.join("waypoints.txt"), core::write_waypoints(&self.waypoints).as_bytes())?;
            }
            self.waypoints_dirty = false;
        }
        Ok(())
    }

    /// Into another world or dimension: the old one saved, the new one's
    /// waypoints read; its regions are read as they come into view.
    fn switch(&mut self, to: Option<(PathBuf, String)>) {
        let _ = self.save();
        self.surface.clear();
        self.cave.clear();
        self.cave_y = None;
        self.scanned.clear();
        self.loaded.clear();
        self.alive = None;
        self.last_feet = None;
        self.waypoints = to
            .as_ref()
            .and_then(|(dir, _)| std::fs::read_to_string(dir.join("waypoints.txt")).ok())
            .map(|t| core::read_waypoints(&t))
            .unwrap_or_default();
        self.waypoints_dirty = false;
        self.world = to;
    }

    fn dim(&self) -> String {
        self.world.as_ref().map(|w| w.1.clone()).unwrap_or_default()
    }

    /// The map shown now: the cave's under a roof.
    fn map(&self) -> &Map {
        if self.in_cave {
            &self.cave
        } else {
            &self.surface
        }
    }
}

/// Written whole or not at all: to a side file, then over the old one.
fn write(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// What the block at (x, y, z) is to the map; None for air.
fn look_at(game: &super::Game<'_>, looks: &mut HashMap<u32, Look>, x: i32, y: i32, z: i32) -> Option<Look> {
    let facts = game.res.facts();
    let state = game.world.block(x, y, z);
    if facts.is_air(state) {
        return None;
    }
    // UNVERIFIED: a state's id, as Eksen's `parse_state(..).id()` reads it;
    // `state_string` is Eksen's guess too.
    let key = u32::from(state.id());
    Some(*looks.entry(key).or_insert_with(|| {
        let text: String = facts.state_string(state);
        core::look(&text)
    }))
}

fn cell(game: &super::Game<'_>, looks: &mut HashMap<u32, Look>, x: i32, y: i32, z: i32) -> Cell {
    match look_at(game, looks, x, y, z) {
        None | Some(Look::Clear) => Cell::Air,
        Some(Look::Water) => Cell::Water,
        Some(Look::Block(rgb, _)) => Cell::Block(rgb),
    }
}

/// Something over the player's head hides the sky (leaves and logs do not
/// count, nor water): the cave map is shown.
fn under_roof(game: &super::Game<'_>, looks: &mut HashMap<u32, Look>, feet: [i32; 3], max: i32) -> bool {
    let top = (feet[1] + 48).min(max);
    (feet[1] + 2..=top).any(|y| matches!(look_at(game, looks, feet[0], y, feet[2]), Some(Look::Block(_, true))))
}

/// A chunk's columns from `top` down; `old` gives each column a hint where
/// its top was, so a rescan does not read the empty sky again.
fn scan_chunk(game: &super::Game<'_>, looks: &mut HashMap<u32, Look>, cx: i32, cz: i32, top: i32, min: i32, old: Option<&core::Cols>) -> core::Cols {
    let mut cols = core::empty();
    for (i, col) in cols.iter_mut().enumerate() {
        let (x, z) = (cx * 16 + (i as i32 & 15), cz * 16 + (i as i32 >> 4));
        let hint = old.map(|o| o[i]).filter(|c| c.known()).map(|c| i32::from(c.height));
        *col = core::scan_hinted(top, min, hint, &mut |y| cell(game, looks, x, y, z));
    }
    cols
}

/// A few chunks near the player that are due, nearest first.
fn scan(game: &super::Game<'_>, s: &mut State, settings: &Settings, t: u64) {
    let Some(p) = game.player.as_ref() else { return };
    let feet = [p.position.x.floor() as i32, p.position.y.floor() as i32, p.position.z.floor() as i32];
    let (min, max) = core::heights(&s.dim());
    let cave = match settings.cave {
        core::Cave::On => true,
        core::Cave::Off => false,
        core::Cave::Auto => under_roof(game, &mut s.looks, feet, max),
    };
    if cave && s.cave_y.map_or(true, |y| (y - feet[1]).abs() > 3) {
        // Another floor: the cave map starts again from here.
        s.cave.clear();
        s.cave_y = Some(feet[1]);
        s.scanned.retain(|k, _| !k.2);
    }
    s.in_cave = cave;
    let (pcx, pcz) = (feet[0] >> 4, feet[2] >> 4);
    let mut done = 0;
    for &(dx, dz) in offsets() {
        if done >= settings.speed.max(1) {
            break;
        }
        let (cx, cz) = (pcx + dx, pcz + dz);
        if !game.world.chunks.contains_key(&(cx, cz)) {
            continue;
        }
        let near = dx.abs() <= 1 && dz.abs() <= 1;
        let every = if near { NEAR_MS } else { FAR_MS };
        if s.scanned.get(&(cx, cz, cave)).is_some_and(|at| t.saturating_sub(*at) < every) {
            continue;
        }
        s.scanned.insert((cx, cz, cave), t);
        done += 1;
        if cave {
            let top = (feet[1] + 1).min(max);
            let cols = scan_chunk(game, &mut s.looks, cx, cz, top, min, None);
            s.cave.put(cx, cz, cols);
        } else {
            s.ensure(cx >> 5, cz >> 5);
            // Only chunks rescanned often trust the hint; the rest are read
            // whole, so a roof built high up is found in the end.
            let State { surface, looks, .. } = &mut *s;
            let old = if near { surface.chunks.get(&(cx, cz)) } else { None };
            let cols = scan_chunk(game, looks, cx, cz, max, min, old);
            surface.put(cx, cz, cols);
        }
    }
}

/// The other players and the mobs, for the radar.
fn dots(game: &super::Game<'_>) -> Vec<core::Dot> {
    let me = game.remote.as_ref().map(|r| r.entity_id as i64);
    let names = game.remote.as_ref().map(|r| &r.level_player_names);
    game.mobs
        .iter()
        .filter(|m| m.alive() && Some(m.id as i64) != me)
        .map(|m| {
            let name = names.and_then(|n| n.get(&i32::try_from(m.id).ok()?).cloned());
            let kind = if name.is_some() { core::Kind::Player } else { core::kind(&m.kind.id) };
            core::Dot { at: [m.position.x, m.position.y, m.position.z], kind, name }
        })
        .collect()
}

fn feet(game: &super::Game<'_>) -> Option<[f64; 3]> {
    game.player.as_ref().map(|p| [p.position.x, p.position.y, p.position.z])
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

// ---- hooks -------------------------------------------------------------------

/// Each frame: the world followed, chunks scanned, a death noticed, the map
/// saved now and then; the world map's commands sent and its close asked.
pub(crate) fn frame(game: &mut super::Game<'_>) {
    let open = game.native_mod_screen() == Some(ID);
    let (close, commands) = {
        let mut s = state();
        s.screen.open = open;
        if !open {
            s.screen.released();
            s.screen.menu = None;
        }
        (std::mem::take(&mut s.screen.close_asked), std::mem::take(&mut s.commands))
    };
    if close && open {
        game.native_mod_screen_close(ID);
    }
    for c in commands {
        if let Err(e) = game.native_send_chat(&c) {
            game.native_chat(&format!("{PREFIX}Komut gönderilemedi: {e}"));
        }
    }
    if !game.native_on(ID) {
        return;
    }
    let game = &*game;
    let t = now();
    let get = |k: &str| game.native_setting(ID, k);
    let settings = Settings::read(&get);
    let mut guard = state();
    let s = &mut *guard;
    let place = world_of(game, &s.sub);
    if s.world != place {
        s.switch(place);
    }
    if s.world.is_none() {
        return;
    }
    scan(game, s, &settings, t);

    // A death: a waypoint where it happened, in place of the last one.
    let alive = horizon_alive(game);
    if s.alive == Some(true) && alive == Some(false) {
        if let Some(f) = s.last_feet {
            s.waypoints.retain(|w| !w.death);
            s.waypoints.push(Waypoint { name: "Ölüm".into(), at: f.map(|v| v.floor() as i32), colour: 0xAAAAAA, dimension: s.dim(), death: true });
            s.waypoints_dirty = true;
        }
    }
    if alive == Some(true) {
        s.last_feet = feet(game);
    }
    if alive.is_some() {
        s.alive = alive;
    }
    if t.saturating_sub(s.saved_at) >= SAVE_MS {
        s.saved_at = t;
        let _ = s.save();
    }
}

/// The HUD: the minimap, and the waypoints where they are in the world.
pub(crate) fn hud(game: &mut super::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    if !game.native_on(ID) || game.native_mod_screen() == Some(ID) {
        return;
    }
    let game = &*game;
    let get = |k: &str| game.native_setting(ID, k);
    let settings = Settings::read(&get);
    let Some(me) = feet(game) else { return };
    let yaw = game.native_player().map_or(0.0, |p| p.yaw);
    let dots = if settings.radar && settings.minimap { dots(game) } else { Vec::new() };
    let screen = [batch.width, batch.height];
    let measure = |s: &str| game.native_text_width(s);
    let draws = {
        let s = state();
        let dim = s.dim();
        let wps: Vec<&Waypoint> = s.waypoints.iter().filter(|w| w.dimension == dim).collect();
        let mut out = Vec::new();
        if settings.in_world {
            if let Some((cam, cyaw, cpitch, fov)) = game.locator_camera {
                for w in &wps {
                    let pos = [f64::from(w.at[0]) + 0.5, f64::from(w.at[1]) + 1.0, f64::from(w.at[2]) + 0.5];
                    let Some(at) = core::project(cam, cyaw, cpitch, fov, screen, pos) else { continue };
                    if at[0] < 0.0 || at[1] < 0.0 || at[0] > screen[0] || at[1] > screen[1] {
                        continue;
                    }
                    let d = ((pos[0] - me[0]).powi(2) + (pos[1] - me[1]).powi(2) + (pos[2] - me[2]).powi(2)).sqrt();
                    out.extend(core::label(w, at, d, &measure));
                }
            }
        }
        if settings.minimap {
            let mini = core::Mini { settings: &settings, map: s.map(), me, yaw, dots: &dots, waypoints: &wps, screen, cave: s.in_cave };
            out.extend(core::minimap(&mini, &measure));
        }
        out
    };
    paint(game, batch, &draws);
}

/// Keys in the world: M the map, B a new waypoint, = and - the zoom.
pub(crate) fn key(game: &mut super::Game<'_>, key: &str) {
    let is = |game: &super::Game<'_>, i: usize| game.native_mod_key(ID, KEYS[i].0, KEYS[i].2) == key;
    if is(game, 0) || is(game, 1) {
        let new = is(game, 1);
        let Some(me) = feet(game) else { return };
        if game.native_mod_screen_open(ID) {
            let mut s = state();
            let n = s.waypoints.len();
            s.screen.opened(me, new, n);
        }
    } else if is(game, 2) || is(game, 3) {
        let now = Settings::read(&|k| game.native_setting(ID, k)).zoom;
        let next = core::step_zoom(&core::ZOOMS, now, is(game, 2));
        game.native_set_setting(ID, "zoom", &next.to_string());
    }
}

/// The world map screen drawn.
pub(crate) fn screen(game: &super::Game<'_>, batch: &mut horizon_client::hud::HudBatch, mouse: [f32; 2]) {
    let size = [batch.width, batch.height];
    let me = feet(game).unwrap_or([0.0; 3]);
    let yaw = game.native_player().map_or(0.0, |p| p.yaw);
    let dots = dots(game);
    let measure = |s: &str| game.native_text_width(s);
    let draws = {
        let mut guard = state();
        let s = &mut *guard;
        s.screen.size = size;
        if s.screen.follow {
            s.screen.centre = [me[0], me[2]];
        }
        // The regions in view, a few each frame.
        let v = s.screen.view();
        let (a, b) = (v.world([0.0, 0.0]), v.world(size));
        let r = |w: f64| (w.floor() as i32) >> 9;
        let mut budget = 6;
        'load: for rx in r(a[0])..=r(b[0]) {
            for rz in r(a[1])..=r(b[1]) {
                if budget == 0 {
                    break 'load;
                }
                if !s.loaded.contains(&(rx, rz)) {
                    s.ensure(rx, rz);
                    budget -= 1;
                }
            }
        }
        let dim = s.dim();
        let world = s.world.as_ref().and_then(|w| w.0.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let title = format!("{NAME} — {world} · {}{}", dim.trim_start_matches("minecraft:"), if s.in_cave { " · mağara" } else { "" });
        let list: Vec<&Waypoint> = s.waypoints.iter().filter(|w| w.dimension == dim).collect();
        s.screen.draw(s.map(), me, yaw, &dots, &list, &title, mouse, &measure)
    };
    paint(game, batch, &draws);
}

/// A button down on the world map.
pub(crate) fn press(game: &super::Game<'_>, at: [f32; 2], button: i32) {
    let _ = game;
    let mut guard = state();
    let s = &mut *guard;
    let dim = s.dim();
    let shown: Vec<usize> = (0..s.waypoints.len()).filter(|i| s.waypoints[*i].dimension == dim).collect();
    let action = {
        let list: Vec<&Waypoint> = shown.iter().map(|i| &s.waypoints[*i]).collect();
        let map = if s.in_cave { &s.cave } else { &s.surface };
        let height = |x: i32, z: i32| map.column(x, z).map(|c| i32::from(c.height));
        s.screen.press(at, button, &list, &height)
    };
    act(s, action, &shown);
}

fn act(s: &mut State, action: core::Action, shown: &[usize]) {
    match action {
        core::Action::Nothing => {}
        core::Action::Command(c) => s.commands.push(c),
        core::Action::Save(name, at, colour) => {
            let dimension = s.dim();
            s.waypoints.push(Waypoint { name, at, colour, dimension, death: false });
            s.waypoints_dirty = true;
        }
        core::Action::Delete(i) => {
            if let Some(&real) = shown.get(i) {
                s.waypoints.remove(real);
                s.waypoints_dirty = true;
            }
        }
    }
}

pub(crate) fn drag(at: [f32; 2]) {
    state().screen.dragged(at);
}

pub(crate) fn released() {
    state().screen.released();
}

pub(crate) fn scroll(at: [f32; 2], dy: f64) {
    state().screen.scrolled(at, dy);
}

pub(crate) fn char(c: char) -> bool {
    state().screen.typed(c)
}

/// A key on the world map: the dialog's, else the map key closes it and
/// = / - zoom. True when taken (Escape is always the game's).
pub(crate) fn screen_key(game: &super::Game<'_>, key: &str) -> bool {
    if key == "key.keyboard.escape" {
        return false;
    }
    let mut guard = state();
    let s = &mut *guard;
    let dim = s.dim();
    let shown: Vec<usize> = (0..s.waypoints.len()).filter(|i| s.waypoints[*i].dimension == dim).collect();
    let (taken, action) = s.screen.key(key);
    if taken {
        act(s, action, &shown);
        return true;
    }
    let is = |i: usize| game.native_mod_key(ID, KEYS[i].0, KEYS[i].2) == key;
    if is(0) {
        s.screen.close_asked = true;
        return true;
    }
    if is(2) || is(3) {
        let mid = s.screen.view().mid();
        s.screen.scrolled(mid, if is(2) { 1.0 } else { -1.0 });
        return true;
    }
    false
}

// ---- `/pusula` -------------------------------------------------------------------

pub(crate) fn command(game: &super::Game<'_>, args: &[&str]) -> Vec<String> {
    let p = PREFIX;
    let get = |k: &str| game.native_setting(ID, k);
    match args {
        [] => vec![
            format!("{p}{NAME} {VERSION}"),
            "M: dünya haritası · B: yeni waypoint · = / -: minimap yakınlaştır".into(),
            "/pusula wp ekle [ad] · wp sil <ad> · wp liste".into(),
            "/pusula ayar [ayar] [değer] · /pusula minimap (aç/kapat) · /pusula dunya [ad]".into(),
        ],
        ["minimap"] => {
            let on = !Settings::read(&get).minimap;
            game.native_set_setting(ID, "minimap", if on { "acik" } else { "kapali" });
            vec![format!("{p}Minimap {}.", if on { "açık" } else { "kapalı" })]
        }
        ["ayar"] | ["ayarlar"] => {
            let mut out = vec![format!("{p}Ayarlar (/pusula ayar <ayar> <değer>):")];
            for (k, d, what) in core::OPTIONS {
                let v = get(k).and_then(|v| core::checked(k, &v).ok()).unwrap_or_else(|| d.to_owned());
                out.push(format!("{k} = {v} — {what}"));
            }
            out
        }
        ["ayar", key, value @ ..] if !value.is_empty() => match core::checked(key, &value.join(" ")) {
            Ok(v) => {
                game.native_set_setting(ID, key, &v);
                vec![format!("{p}{key} = {v}")]
            }
            Err(e) => vec![format!("{p}{e}")],
        },
        ["dunya" | "dünya", name @ ..] => {
            let mut s = state();
            let name = name.join(" ");
            s.sub = (!name.trim().is_empty()).then(|| name.trim().to_owned());
            match &s.sub {
                Some(n) => vec![format!("{p}Bu sunucuda \"{n}\" haritası kullanılıyor (/pusula dunya ile varsayılana dön).")],
                None => vec![format!("{p}Sunucunun varsayılan haritası kullanılıyor.")],
            }
        }
        ["wp", "liste" | "list"] => {
            let s = state();
            let dim = s.dim();
            let me = feet(game).unwrap_or([0.0; 3]);
            let mut out = vec![format!("{p}Waypointler ({}):", dim.trim_start_matches("minecraft:"))];
            for w in s.waypoints.iter().filter(|w| w.dimension == dim) {
                let d = ((f64::from(w.at[0]) - me[0]).powi(2) + (f64::from(w.at[2]) - me[2]).powi(2)).sqrt();
                out.push(format!("{} — {} {} {} ({} m)", w.name, w.at[0], w.at[1], w.at[2], d.round() as i64));
            }
            if out.len() == 1 {
                out.push("Yok. B ya da /pusula wp ekle".into());
            }
            out
        }
        ["wp", "ekle" | "add", name @ ..] => {
            let Some(me) = feet(game) else { return vec![format!("{p}Bir dünyada değilsin.")] };
            let mut s = state();
            if s.world.is_none() {
                return vec![format!("{p}Bir dünyada değilsin.")];
            }
            let n = s.waypoints.len();
            let name = if name.is_empty() { format!("Waypoint {}", n + 1) } else { name.join(" ") };
            let at = me.map(|v| v.floor() as i32);
            let dimension = s.dim();
            s.waypoints.push(Waypoint { name: name.clone(), at, colour: core::COLOURS[n % core::COLOURS.len()], dimension, death: false });
            s.waypoints_dirty = true;
            vec![format!("{p}\"{name}\" eklendi: {} {} {}", at[0], at[1], at[2])]
        }
        ["wp", "sil" | "remove", name @ ..] if !name.is_empty() => {
            let mut s = state();
            let name = name.join(" ").to_lowercase();
            let dim = s.dim();
            let before = s.waypoints.len();
            s.waypoints.retain(|w| !(w.dimension == dim && w.name.to_lowercase() == name));
            let gone = before - s.waypoints.len();
            s.waypoints_dirty |= gone > 0;
            vec![if gone > 0 { format!("{p}{gone} waypoint silindi.") } else { format!("{p}Bu boyutta \"{name}\" adlı waypoint yok.") }]
        }
        _ => vec![format!("{p}Bilinmeyen komut. /pusula yardımı gösterir.")],
    }
}

// ---- Horizon guesses (UNVERIFIED: Horizon's source is private) ------------------
// `state.id()` in `look_at` above, and the one below. If the first build
// disagrees, these are the lines to fix.

/// Alive or dead, as the player's own entry in the mob list has it (Arena
/// HUD's guess too: our entity is in `game.mobs`). None when not found.
fn horizon_alive(game: &super::Game<'_>) -> Option<bool> {
    let me = game.remote.as_ref()?.entity_id as i64;
    let m = game.mobs.iter().find(|m| m.id as i64 == me)?;
    Some(m.alive() && m.health > 0.0)
}
