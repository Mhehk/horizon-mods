// Health Indicators — mhv1 (mihehk): a Horizon port of AdyTech99's Health
// Indicators (https://github.com/AdyTech99/HealthIndicators), licensed like the
// original under the GNU Lesser General Public License v3 (LICENSE beside it).
//
// Hearts (or a number) and armour points float above players and mobs, chosen
// by the original's filters; the targeted entity's name and health can be
// monitored on the HUD, and an arc points at whatever hurt you in single
// player. Settings: `config/health_indicators_config.json` (the original's
// keys and defaults) and `config/healthindicators.json` (the toggles), on
// Options > Mods > Health Indicators (key I, or `/healthindicators`).
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use horizon_client::hud::{HudAtlas, HudBatch};
use horizon_client::mesher::Vertex;
use horizon_client::text::Text;

pub(crate) const ID: &str = "healthindicators";

// ---------------------------------------------------------------------------------------------
// ModConfig (config/health_indicators_config.json)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Kind {
    Bool(&'static str, &'static str),
    Enum(&'static [(&'static str, &'static str)]),
    Int(i64, i64, i64),
    Float(f64, f64, f64, u8),
    Color,
    List,
}

struct Entry {
    section: usize,
    key: &'static str,
    caption: &'static str,
    kind: Kind,
    default: &'static str,
}

const SECTIONS: [&str; 4] = ["Visibility", "Appearance", "Messages & Commands", "Damage Direction Indicators"];
const ON_OFF: Kind = Kind::Bool("ON", "OFF");
const INDICATOR: &[(&str, &str)] = &[("HEARTS", "Hearts"), ("NUMBER", "Number"), ("DYNAMIC", "Dynamic")];
const MESSAGE: &[(&str, &str)] = &[("ACTIONBAR", "Actionbar"), ("CHAT", "Chat"), ("NONE", "None")];

const ENTRIES: &[Entry] = &[
    Entry { section: 0, key: "passive_mobs", caption: "Show for passive mobs", kind: ON_OFF, default: "true" },
    Entry { section: 0, key: "hostile_mobs", caption: "Show for hostile mobs", kind: ON_OFF, default: "true" },
    Entry { section: 0, key: "players", caption: "Show for players", kind: ON_OFF, default: "true" },
    Entry { section: 0, key: "self", caption: "Show for yourself", kind: ON_OFF, default: "false" },
    Entry { section: 0, key: "show_through_walls", caption: "Show through walls", kind: ON_OFF, default: "false" },
    Entry { section: 0, key: "blacklistOrWhitelist", caption: "Blacklist or Whitelist", kind: Kind::Bool("Blacklist", "Whitelist"), default: "true" },
    Entry { section: 0, key: "list", caption: "Filtered Entities", kind: Kind::List, default: "minecraft:armor_stand" },
    Entry { section: 0, key: "after_attack", caption: "Attacked entities only", kind: ON_OFF, default: "false" },
    Entry { section: 0, key: "time_after_hit", caption: "Seconds to reset indicator", kind: Kind::Int(0, 120, 1), default: "60" },
    Entry { section: 0, key: "damaged_only", caption: "Damaged entities only", kind: ON_OFF, default: "false" },
    Entry { section: 0, key: "max_health_percentage", caption: "Health threshold", kind: Kind::Int(0, 100, 1), default: "100" },
    Entry { section: 0, key: "looking_at", caption: "Targeted entities only", kind: ON_OFF, default: "false" },
    Entry { section: 0, key: "reach", caption: "Range", kind: Kind::Int(0, 1024, 1), default: "3" },
    Entry { section: 0, key: "within_distance", caption: "Within distance only", kind: ON_OFF, default: "false" },
    Entry { section: 0, key: "distance", caption: "Distance", kind: Kind::Int(0, 512, 4), default: "64" },
    Entry { section: 0, key: "override_players", caption: "Always show player health", kind: ON_OFF, default: "false" },
    Entry { section: 1, key: "indicator_type", caption: "Indicator Type", kind: Kind::Enum(INDICATOR), default: "HEARTS" },
    Entry { section: 1, key: "dynamic_health_threshold", caption: "Dynamic Health Threshold", kind: Kind::Int(10, 200, 2), default: "100" },
    Entry { section: 1, key: "icons_per_row", caption: "Hearts per row (beta)", kind: Kind::Int(1, 1000, 1), default: "10" },
    Entry { section: 1, key: "use_vanilla_textures", caption: "Use vanilla textures", kind: ON_OFF, default: "false" },
    Entry { section: 1, key: "health_bar_opacity", caption: "Opacity", kind: Kind::Int(0, 100, 1), default: "100" },
    Entry { section: 1, key: "show_heart_effects", caption: "Show effects", kind: ON_OFF, default: "true" },
    Entry { section: 1, key: "number_color", caption: "Text color", kind: Kind::Color, default: "#FF0000" },
    Entry { section: 1, key: "render_number_display_shadow", caption: "Render shadow", kind: ON_OFF, default: "false" },
    Entry { section: 1, key: "decimal_places", caption: "Decimal Places", kind: Kind::Int(0, 12, 1), default: "1" },
    Entry { section: 1, key: "percentage_based_health", caption: "Percentage based health", kind: ON_OFF, default: "false" },
    Entry { section: 1, key: "max_health", caption: "Base health", kind: Kind::Int(1, 100, 1), default: "20" },
    Entry { section: 1, key: "size", caption: "Indicator Size", kind: Kind::Float(0.0, 0.1, 0.005, 3), default: "0.025" },
    Entry { section: 1, key: "display_offset", caption: "Display offset", kind: Kind::Float(-1000.0, 1000.0, 1.0, 1), default: "0.0" },
    Entry { section: 1, key: "offset_step_size", caption: "Offset step size", kind: Kind::Float(0.0, 10.0, 0.5, 1), default: "1.0" },
    Entry { section: 1, key: "force_higher_offset_for_players", caption: "Force higher offset for players", kind: ON_OFF, default: "false" },
    Entry { section: 2, key: "message_type", caption: "Message type", kind: Kind::Enum(MESSAGE), default: "ACTIONBAR" },
    Entry { section: 2, key: "colored_messages", caption: "Colored Messages", kind: ON_OFF, default: "true" },
    Entry { section: 2, key: "enable_commands", caption: "Enable mod commands", kind: Kind::Bool("Yes", "No"), default: "true" },
    Entry { section: 3, key: "enable_damage_direction_indicators", caption: "Enable Damage Direction Indicators", kind: ON_OFF, default: "false" },
    Entry { section: 3, key: "damage_direction_indicators_scale", caption: "Size", kind: Kind::Int(1, 8, 1), default: "2" },
    Entry { section: 3, key: "damage_direction_indicators_color", caption: "Color", kind: Kind::Color, default: "#FF0000" },
    Entry { section: 3, key: "damage_direction_indicators_visibility_time", caption: "Time to disappear", kind: Kind::Int(1, 8, 1), default: "4" },
    Entry { section: 3, key: "damage_direction_indicators_fade_out", caption: "Should Fade Out", kind: ON_OFF, default: "true" },
    Entry { section: 3, key: "damage_direction_indicators_fade_out_time", caption: "Fade Out Time", kind: Kind::Int(0, 7, 1), default: "3" },
];

/// `java.awt.Color`'s named colours, for the colour fields' button.
const COLORS: [(&str, &str); 13] = [
    ("#FF0000", "Red"),
    ("#FFFFFF", "White"),
    ("#000000", "Black"),
    ("#808080", "Gray"),
    ("#404040", "Dark Gray"),
    ("#C0C0C0", "Light Gray"),
    ("#FFC800", "Orange"),
    ("#FFAFAF", "Pink"),
    ("#FFFF00", "Yellow"),
    ("#00FF00", "Green"),
    ("#00FFFF", "Cyan"),
    ("#0000FF", "Blue"),
    ("#FF00FF", "Magenta"),
];

struct Values {
    map: BTreeMap<&'static str, String>,
    stamp: Option<SystemTime>,
    checked: Instant,
}

thread_local! {
    static VALUES: RefCell<Option<Values>> = const { RefCell::new(None) };
}

fn config_dir() -> Option<PathBuf> {
    crate::game::Game::native_config_dir().ok()
}

fn path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("health_indicators_config.json"))
}

fn stamp() -> Option<SystemTime> {
    std::fs::metadata(path()?).and_then(|m| m.modified()).ok()
}

fn defaults() -> BTreeMap<&'static str, String> {
    ENTRIES.iter().map(|e| (e.key, e.default.to_owned())).collect()
}

fn from_json(e: &Entry, v: &serde_json::Value) -> Option<String> {
    Some(match e.kind {
        Kind::Bool(..) => v.as_bool()?.to_string(),
        Kind::Enum(names) => {
            let s = v.as_str()?;
            names.iter().any(|(n, _)| *n == s).then(|| s.to_owned())?
        }
        Kind::Int(..) => v.as_i64()?.to_string(),
        Kind::Float(..) => format!("{:?}", v.as_f64()?),
        Kind::Color => {
            if let Some(i) = v.as_i64() {
                format!("#{:06X}", i & 0xFF_FFFF)
            } else {
                v.as_str()?.to_owned()
            }
        }
        Kind::List => v
            .as_array()?
            .iter()
            .filter_map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(","),
    })
}

fn to_json(e: &Entry, v: &str) -> String {
    match e.kind {
        Kind::Enum(_) | Kind::Color => format!("\"{v}\""),
        Kind::List => {
            let items: Vec<String> = v
                .split(',')
                .filter(|s| !s.trim().is_empty())
                .map(|s| serde_json::to_string(s.trim()).unwrap_or_default())
                .collect();
            format!("[{}]", items.join(", "))
        }
        _ => v.to_owned(),
    }
}

fn read_file(map: &mut BTreeMap<&'static str, String>) -> bool {
    let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) else {
        return false;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    for e in ENTRIES {
        if let Some(v) = json.get(e.key).and_then(|v| from_json(e, v)) {
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
        .map(|e| format!("  \"{}\": {}", e.key, to_json(e, map.get(e.key).map_or(e.default, String::as_str))))
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
            Values { map, stamp: stamp(), checked: Instant::now() }
        });
        f(v)
    })
}

fn get(key: &str) -> String {
    with_values(|v| v.map.get(key).cloned()).unwrap_or_default()
}
fn on(key: &str) -> bool {
    get(key) == "true"
}
fn int(key: &str) -> i64 {
    get(key).parse::<f64>().map_or(0, |f| f as i64)
}
fn float(key: &str) -> f64 {
    get(key).parse::<f64>().unwrap_or(0.0)
}
fn color(key: &str) -> u32 {
    u32::from_str_radix(get(key).trim().trim_start_matches('#'), 16).unwrap_or(0xFF_0000) & 0xFF_FFFF
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

// ---------------------------------------------------------------------------------------------
// Config (config/healthindicators.json): the toggles the keys flip
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Toggles {
    hearts: bool,
    armor: bool,
    override_all: bool,
}

thread_local! {
    static TOGGLES: Cell<Option<Toggles>> = const { Cell::new(None) };
}

fn toggles() -> Toggles {
    if let Some(t) = TOGGLES.get() {
        return t;
    }
    let mut t = Toggles { hearts: true, armor: false, override_all: false };
    if let Some(text) = config_dir().and_then(|d| std::fs::read_to_string(d.join("healthindicators.json")).ok())
        && let Ok(j) = serde_json::from_str::<serde_json::Value>(&text)
    {
        let b = |k: &str, d: bool| j.get(k).and_then(|v| v.as_bool()).unwrap_or(d);
        t = Toggles {
            hearts: b("heartsRenderingEnabled", true),
            armor: b("armorRenderingEnabled", false),
            override_all: b("overrideAllFiltersEnabled", false),
        };
    }
    TOGGLES.set(Some(t));
    t
}

fn save_toggles(t: Toggles) {
    TOGGLES.set(Some(t));
    if let Some(d) = config_dir() {
        let _ = std::fs::write(
            d.join("healthindicators.json"),
            format!(
                "{{\"heartsRenderingEnabled\":{},\"armorRenderingEnabled\":{},\"overrideAllFiltersEnabled\":{}}}",
                t.hearts, t.armor, t.override_all
            ),
        );
    }
}

// ---------------------------------------------------------------------------------------------
// RenderTracker
// ---------------------------------------------------------------------------------------------

struct Tracker {
    /// Entity id -> ticks left (`UUIDS`).
    shown: HashMap<u32, i64>,
    after_attack: bool,
    /// `/healthindicators monitor`: the entity shown on the HUD.
    monitored: Option<u32>,
    /// `DamageDirectionIndicatorRenderer`.
    since_damage: Option<u32>,
    attacker: Option<u32>,
    /// Keys held and clicks this tick, by key name.
    held: HashSet<&'static str>,
    clicks: Vec<&'static str>,
}

thread_local! {
    static TRACKER: RefCell<Tracker> = RefCell::new(Tracker {
        shown: HashMap::new(),
        after_attack: false,
        monitored: None,
        since_damage: None,
        attacker: None,
        held: HashSet::new(),
        clicks: Vec::new(),
    });
    static SECTION: Cell<Option<usize>> = const { Cell::new(None) };
}

fn is_player(m: &horizon_client::mobs::Mob) -> bool {
    &*m.kind.id == "minecraft:player"
}

/// The name a player goes by (`getName`), from the server's player list.
fn player_name(game: &crate::game::Game<'_>, m: &horizon_client::mobs::Mob) -> Option<String> {
    let r = game.remote.as_ref()?;
    r.level_player_names.get(&i32::try_from(m.id).ok()?).cloned()
}

fn max_health(m: &horizon_client::mobs::Mob) -> f32 {
    horizon_client::mobs::finalize::max_health(m)
}

/// `RenderTracker.isInvalid`.
fn invalid(game: &crate::game::Game<'_>, m: &horizon_client::mobs::Mob) -> bool {
    !m.alive() || game.player.is_none() || game.riding == Some(m.id) || (m.is_invisible() && !game.spectator())
}

/// `isEntityTypeAllowed`: passive (ageable) mobs, monsters and players.
fn type_allowed(m: &horizon_client::mobs::Mob) -> bool {
    let category = m.kind.category.as_str();
    let passive = matches!(category, "creature" | "water_creature" | "underground_water_creature" | "axolotls");
    if !on("passive_mobs") && passive && !is_player(m) {
        return false;
    }
    if !on("hostile_mobs") && category == "monster" {
        return false;
    }
    if !on("players") && is_player(m) {
        return false;
    }
    true
}

/// `isTargeted`: the entity the camera's look meets first within `reach`, before any block.
fn targeted(game: &crate::game::Game<'_>, m: &horizon_client::mobs::Mob) -> bool {
    let reach = int("reach").max(0) as f64;
    let Some((eye, dir)) = game.aim() else {
        return false;
    };
    let b = m.hitbox();
    let (lo, hi) = (b.min_position(), b.max_position());
    let (lo, hi) = ([lo.x, lo.y, lo.z], [hi.x, hi.y, hi.z]);
    let (mut t0, mut t1) = (0.0f64, reach);
    for a in 0..3 {
        if dir[a].abs() < 1e-12 {
            if eye[a] < lo[a] || eye[a] > hi[a] {
                return false;
            }
            continue;
        }
        let (mut n, mut f) = ((lo[a] - eye[a]) / dir[a], (hi[a] - eye[a]) / dir[a]);
        if n > f {
            std::mem::swap(&mut n, &mut f);
        }
        t0 = t0.max(n);
        t1 = t1.min(f);
        if t0 > t1 {
            return false;
        }
    }
    if !game.before_block(t0) {
        return false;
    }
    // The nearest such entity only.
    !game.mobs.iter().any(|o| {
        if o.id == m.id || !o.alive() {
            return false;
        }
        let b = o.hitbox();
        let (lo, hi) = (b.min_position(), b.max_position());
        let (lo, hi) = ([lo.x, lo.y, lo.z], [hi.x, hi.y, hi.z]);
        let (mut a0, mut a1) = (0.0f64, t0);
        for a in 0..3 {
            if dir[a].abs() < 1e-12 {
                if eye[a] < lo[a] || eye[a] > hi[a] {
                    return false;
                }
                continue;
            }
            let (mut n, mut f) = ((lo[a] - eye[a]) / dir[a], (hi[a] - eye[a]) / dir[a]);
            if n > f {
                std::mem::swap(&mut n, &mut f);
            }
            a0 = a0.max(n);
            a1 = a1.min(f);
            if a0 > a1 {
                return false;
            }
        }
        a0 < t0
    })
}

fn distance_to_player(game: &crate::game::Game<'_>, m: &horizon_client::mobs::Mob) -> f64 {
    game.player.as_ref().map_or(f64::MAX, |p| {
        let d = [m.position.x - p.position.x, m.position.y - p.position.y, m.position.z - p.position.z];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    })
}

/// `satisfiesAdvancedCriteria`.
fn advanced(game: &crate::game::Game<'_>, t: &Tracker, m: &horizon_client::mobs::Mob) -> bool {
    if on("override_players") && is_player(m) {
        return true;
    }
    if !type_allowed(m) {
        return false;
    }
    if on("after_attack") && !t.shown.contains_key(&m.id) {
        return false;
    }
    let max = max_health(m);
    if on("damaged_only")
        && (m.health == max || m.health > max * (int("max_health_percentage") as f32 / 100.0))
        && m.absorption <= 0.0
    {
        return false;
    }
    if on("looking_at") && !targeted(game, m) {
        return false;
    }
    if on("within_distance") && distance_to_player(game, m) > int("distance") as f64 {
        return false;
    }
    !invalid(game, m)
}

/// `satisfiesList`: the blacklist or whitelist, by entity type or a player's name.
fn in_list(game: &crate::game::Game<'_>, m: &horizon_client::mobs::Mob) -> bool {
    let list: Vec<String> = get("list").split(',').map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect();
    let blacklist = on("blacklistOrWhitelist");
    if !blacklist {
        if list.is_empty() {
            return true;
        }
        if list.iter().any(|s| s == "minecraft:player") && is_player(m) {
            return true;
        }
    }
    let name = if is_player(m) { player_name(game, m) } else { None };
    let matches = |s: &String| {
        if is_player(m) {
            name.as_deref() == Some(s.as_str())
        } else {
            *s == *m.kind.id
        }
    };
    if blacklist { !list.iter().any(matches) } else { list.iter().any(matches) }
}

fn message(game: &mut crate::game::Game<'_>, text: &str, color: Option<u32>) {
    let mut t = Text::literal(text);
    if let Some(c) = color.filter(|_| on("colored_messages")) {
        t = t.with_color(c);
    }
    match get("message_type").as_str() {
        "NONE" => {}
        "CHAT" => {
            let focused = game.screen == crate::game::Screen::Chat;
            let ticks = game.visual_ticks();
            let atlas = &game.gui.atlas;
            game.ui.chat.add(atlas, t, None, horizon_client::chat::ChatSource::Client, ticks, focused);
        }
        _ => game.send_overlay_message(t),
    }
}

const KEYS: [(&str, &str, &str); 6] = [
    ("renderingEnabled", "Toggle Rendering", "key.keyboard.left"),
    ("armorRenderingEnabled", "Toggle Armor Points Rendering", "key.keyboard.right.shift"),
    ("overrideAllFilters", "Override all config filters", "key.keyboard.right"),
    ("increaseHeartOffset", "Increase Heart Offset", "key.keyboard.up"),
    ("decreaseHeartOffset", "Decrease Heart Offset", "key.keyboard.down"),
    ("openModMenuConfig", "Open Config Screen", "key.keyboard.i"),
];

pub(crate) fn keys() -> Vec<[String; 3]> {
    KEYS.iter().map(|(n, l, d)| [(*n).into(), (*l).into(), (*d).into()]).collect()
}

/// A key went down or up: the mod's mappings bound to it hold or click.
pub(crate) fn key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    if !game.native_on(ID) || !game.native_window() || key == "key.keyboard.unknown" {
        return;
    }
    let playing = game.native_playing();
    let names: Vec<&'static str> = KEYS
        .iter()
        .filter(|(n, _, d)| game.native_mod_key(ID, n, d) == key)
        .map(|(n, _, _)| *n)
        .collect();
    TRACKER.with(|t| {
        let mut t = t.borrow_mut();
        for n in names {
            if pressed && playing {
                t.held.insert(n);
                t.clicks.push(n);
            } else if !pressed {
                t.held.remove(n);
            }
        }
    });
}

fn open_settings(game: &mut crate::game::Game<'_>) {
    game.list_native_mods();
    if let Some(i) = game.settings.mods.installed.iter().position(|m| m.id == ID) {
        SECTION.set(None);
        game.open_options_at(false, horizon_client::options_ui::Page::NativeMod(i));
    }
}

fn change_offset(game: &mut crate::game::Game<'_>, by: f64) {
    let v = float("display_offset") + by;
    set("display_offset", format!("{v:?}"));
    message(game, &format!("Set heart offset to {}", (v * 100.0).floor() / 100.0), None);
}

/// End of each client tick: the keys, `RenderTracker.tick` and the damage direction.
pub(crate) fn tick(game: &mut crate::game::Game<'_>) {
    if !game.native_window() {
        return;
    }
    watch(Instant::now());
    if !game.native_on(ID) {
        return;
    }
    let (clicks, override_held) = TRACKER.with(|t| {
        let mut t = t.borrow_mut();
        if !game.native_playing() {
            t.held.clear();
        }
        (std::mem::take(&mut t.clicks), t.held.contains("overrideAllFilters"))
    });
    for c in clicks {
        match c {
            "renderingEnabled" => {
                let mut t = toggles();
                t.hearts = !t.hearts;
                save_toggles(t);
                let text = format!("{} Health Indicators", if t.hearts { "Enabled" } else { "Disabled" });
                message(game, &text, Some(if t.hearts { 0x55FF55 } else { 0xFF5555 }));
            }
            "armorRenderingEnabled" => {
                let mut t = toggles();
                t.armor = !t.armor;
                save_toggles(t);
                let text = format!("{} Armor Indicators", if t.armor { "Enabled" } else { "Disabled" });
                message(game, &text, Some(if t.armor { 0x55FF55 } else { 0xFF5555 }));
            }
            "increaseHeartOffset" => change_offset(game, float("offset_step_size")),
            "decreaseHeartOffset" => change_offset(game, -float("offset_step_size")),
            "openModMenuConfig" => open_settings(game),
            _ => {}
        }
    }
    let mut t = toggles();
    if override_held {
        if !t.override_all {
            t.override_all = true;
            save_toggles(t);
        }
        if get("message_type") != "NONE" {
            game.send_overlay_message(Text::literal(" Config Criteria Temporarily Overridden"));
        }
    } else if t.override_all {
        t.override_all = false;
        save_toggles(t);
        game.send_overlay_message(Text::literal(""));
    }
    let rendering = t.hearts || t.armor;
    TRACKER.with(|cell| {
        let mut tr = cell.borrow_mut();
        if rendering {
            for m in game.mobs.iter() {
                if advanced(game, &tr, m) && in_list(game, m) {
                    let ticks = if on("after_attack") { int("time_after_hit") * 20 } else { 2400 };
                    tr.shown.entry(m.id).or_insert(ticks);
                } else {
                    tr.shown.remove(&m.id);
                }
            }
        }
        // `trimEntities`.
        tr.shown.retain(|_, v| {
            *v -= 1;
            *v > 0
        });
        tr.shown.retain(|id, _| {
            rendering && game.mobs.iter().find(|m| m.id == *id).is_some_and(|m| !invalid(game, m))
        });
        if tr.shown.len() >= 1536 {
            tr.shown.clear();
        }
        let after = on("after_attack");
        if after != tr.after_attack {
            tr.shown.clear();
            tr.after_attack = after;
        }
        if tr.monitored.is_some_and(|id| !game.mobs.iter().any(|m| m.id == id && m.alive())) {
            tr.monitored = None;
        }
        // `DamageDirectionIndicatorRenderer.tick`.
        if let Some(n) = tr.since_damage.as_mut() {
            *n += 1;
        }
        if tr.attacker.is_none_or(|id| !game.mobs.iter().any(|m| m.id == id && m.alive())) {
            tr.since_damage = None;
            tr.attacker = None;
        }
    });
}

/// `EntityDamageMixin`: an entity's damage event, with its attacker.
pub(crate) fn damage(game: &crate::game::Game<'_>, target: i32, cause: Option<i32>) {
    if !game.native_on(ID) || !game.native_window() {
        return;
    }
    let me = game.remote.as_ref().map(|r| r.entity_id);
    let by_player = cause.is_some_and(|c| {
        Some(c) == me
            || game.remote.as_ref().is_some_and(|r| r.level_players.contains_key(&c))
    });
    TRACKER.with(|cell| {
        let mut tr = cell.borrow_mut();
        if by_player && on("after_attack")
            && let Some(m) = game.mobs.iter().find(|m| m.id as i32 == target)
            && type_allowed(m)
            && in_list(game, m)
        {
            tr.shown.insert(m.id, int("time_after_hit") * 20);
        }
        if Some(target) == me
            && let Some(c) = cause
            && game.mobs.iter().any(|m| m.id as i32 == c)
        {
            tr.since_damage = Some(0);
            tr.attacker = Some(c as u32);
        }
    });
}

// ---------------------------------------------------------------------------------------------
// EntityRendererMixin: hearts, the number and armour points above entities
// ---------------------------------------------------------------------------------------------

fn sprite(atlas: &HudAtlas, name: &str) -> Option<horizon_client::hud::Rect> {
    // GUI sprites are keyed by their atlas path (`hud_layout`'s `gui/sprites/`).
    atlas.sprites.get(format!("gui/sprites/{name}").as_str()).copied()
}

/// Quads of `batch` (text space: x right, y down, in pixels of `unit` blocks)
/// as world vertices at `o`, facing the camera.
#[allow(clippy::too_many_arguments)]
fn emit(
    batch: &HudBatch,
    o: glam::Vec3,
    right: glam::Vec3,
    up: glam::Vec3,
    shift: glam::Vec3,
    layer: u32,
    light: u32,
    out: &mut Vec<Vertex>,
) {
    let at = |x: f32, y: f32| o + right * x - up * y;
    for quad in batch.vertices.as_chunks::<6>().0 {
        for v in [quad[5], quad[2], quad[1], quad[0]] {
            out.push(Vertex {
                normal: [0; 4],
                pos: (at(v.pos[0], v.pos[1]) + shift).to_array(),
                uv: v.uv,
                layer,
                color: v.color,
                light,
                glint_uv: [0.0, 0.0],
            });
        }
    }
}

/// The hearts' sprite: effects (wither, poison, frozen) and hardcore when shown.
fn heart_sprite(game: &crate::game::Game<'_>, m: &horizon_client::mobs::Mob, icon: &str, effects: bool) -> String {
    let mut prefix = String::new();
    if effects && on("show_heart_effects") {
        if m.has_effect("minecraft:wither") {
            prefix.push_str("withered_");
        } else if m.has_effect("minecraft:poison") {
            prefix.push_str("poisoned_");
        } else if m.is_fully_frozen() {
            prefix.push_str("frozen_");
        }
        if game.level.hardcore {
            prefix.push_str("hardcore_");
        }
    }
    format!("hud/heart/{prefix}{icon}")
}

/// `shouldShowName` (a name tag drawn above the entity), or forced for other players.
fn name_shown(game: &crate::game::Game<'_>, m: &horizon_client::mobs::Mob, d: f64) -> bool {
    let shown = if is_player(m) {
        !m.is_invisible()
    } else {
        m.custom_name_text.is_some() && m.name_visible
    };
    (shown || (on("force_higher_offset_for_players") && is_player(m))) && d <= 4096.0 && game.player.is_some()
}

fn health_text(m: &horizon_client::mobs::Mob) -> String {
    let places = int("decimal_places").clamp(0, 12) as usize;
    let (health, max, absorption) = (m.health, max_health(m), m.absorption);
    if on("percentage_based_health") {
        format!("{:.*} %", places, (health + absorption) / max * 100.0)
    } else {
        format!("{:.*} / {:.*}", places, health + absorption, places, max)
    }
}

/// Before the name tags: every shown entity's indicator.
pub(crate) fn geometry(
    game: &mut crate::game::Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<Vertex>,
    see_through: &mut Vec<Vertex>,
) {
    if !game.native_on(ID) || !game.native_window() {
        return;
    }
    let t = toggles();
    if !(t.hearts || t.armor || t.override_all) {
        return;
    }
    let (yaw, pitch) = match game.posed_rotation {
        Some(r) => r,
        None if game.camera_type == crate::game::perspective::CameraType::ThirdPersonFront => {
            (game.yaw + 180.0, -game.pitch)
        }
        None => (game.yaw, game.pitch),
    };
    let forward = glam::DVec3::from_array(crate::game::view_vector(yaw, pitch));
    let right = forward.cross(glam::DVec3::Y).normalize_or_zero();
    let up = right.cross(forward).normalize_or_zero();
    let scale = float("size") as f32;
    let (r, u) = (right.as_vec3() * scale, up.as_vec3() * scale);
    let away = forward.as_vec3() * 0.002;
    let opacity = (int("health_bar_opacity").clamp(0, 100) * 255 / 100) as u32;
    let white = opacity << 24 | 0xFF_FFFF;
    let light = horizon_client::block_entity_render::vertex_light(0xFF);
    let shown: Vec<u32> = TRACKER.with(|t| t.borrow().shown.keys().copied().collect());
    let mut numbers = Vec::new();
    for m in game.mobs.iter() {
        // `RenderTracker.isInvalid` is asked again here: a mob that died while the game was paused.
        let listed = shown.contains(&m.id) && !invalid(game, m);
        if !(listed || (t.override_all && !invalid(game, m))) {
            continue;
        }
        let pos = glam::DVec3::new(
            m.previous.x + (m.position.x - m.previous.x) * alpha,
            m.previous.y + (m.position.y - m.previous.y) * alpha,
            m.previous.z + (m.position.z - m.previous.z) * alpha,
        );
        let b = m.hitbox();
        let height = b.max_position().y - b.min_position().y;
        let d = (pos - camera).length_squared();
        let through = on("show_through_walls") && !is_player(m) && targeted(game, m);
        let below_name = is_player(m)
            && d < 100.0
            && game
                .score
                .board
                .display_objective(horizon_client::scoreboard::SLOT_BELOW_NAME)
                .is_some();
        let lift_name = if name_shown(game, m, d) {
            9.0 * 1.15 * f64::from(scale) * if below_name { 2.0 } else { 1.0 }
        } else {
            0.0
        };
        let offset = float("display_offset") * f64::from(scale);
        let max = max_health(m);
        let kind = get("indicator_type");
        let number = kind == "NUMBER" || (kind == "DYNAMIC" && max > int("dynamic_health_threshold") as f32);
        if t.hearts || t.override_all {
            if number {
                let o = pos + glam::DVec3::new(0.0, f64::from(height) + 0.5 + lift_name, 0.0) - camera + up * offset;
                numbers.push((o, m.id, through));
            } else {
                // `renderHearts`.
                let per_row = int("icons_per_row").max(1);
                let (mut red, mut max_i, yellow) = (
                    m.health.ceil() as i64,
                    max.ceil() as i64,
                    m.absorption.ceil() as i64,
                );
                if on("percentage_based_health") {
                    red = ((red as f32 / max_i as f32) * float("max_health") as f32).ceil() as i64;
                    max_i = float("max_health").ceil() as i64;
                }
                let hearts_red = (red as f32 / 2.0).ceil() as i64;
                let last_red_half = red & 1 == 1;
                let hearts_normal = (max_i as f32 / 2.0).ceil() as i64;
                let hearts_yellow = (yellow as f32 / 2.0).ceil() as i64;
                let last_yellow_half = yellow & 1 == 1;
                let total = hearts_normal + hearts_yellow;
                let max_x = (total.min(per_row) * 8 + 1) as f32 / 2.0;
                let density = 50.0 - ((4.0 - (total as f64 / per_row as f64).ceil()).max(-3.0) * 5.0);
                let jumping = (m.has_effect("minecraft:regeneration") && on("show_heart_effects"))
                    .then(|| (f64::from(m.age) % f64::from(max.max(1.0))) as i64);
                let mut row_lift = 0.0;
                for heart in 0..total {
                    if heart % per_row == 0 {
                        row_lift = heart as f64 / density;
                    }
                    let jump = if jumping == Some(heart) { 1.15 * f64::from(scale) } else { 0.0 };
                    let o = pos + glam::DVec3::new(0.0, f64::from(height) + 0.5 + row_lift + jump + lift_name, 0.0)
                        - camera
                        + up * offset;
                    let x = (heart % per_row) as f32 * 8.0 - max_x;
                    let mut empty = HudBatch::new([1, 1]);
                    if let Some(rect) = sprite(&game.gui.atlas, "hud/heart/container") {
                        empty.rect_uv(x, 0.0, 9.0, 9.0, rect, white);
                    }
                    let icon = if heart < hearts_red {
                        Some(if heart == hearts_red - 1 && last_red_half { "half" } else { "full" })
                    } else if heart < hearts_normal {
                        None
                    } else {
                        Some(if heart == total - 1 && last_yellow_half { "absorbing_half" } else { "absorbing_full" })
                    };
                    let mut full = HudBatch::new([1, 1]);
                    if let Some(icon) = icon {
                        let effects = !icon.starts_with("absorbing");
                        let name = heart_sprite(game, m, icon, effects);
                        if let Some(rect) = sprite(&game.gui.atlas, &name).or_else(|| sprite(&game.gui.atlas, &format!("hud/heart/{icon}"))) {
                            full.rect_uv(x, 0.0, 9.0, 9.0, rect, white);
                        }
                    }
                    let (target, layer, l) = if through { (&mut *see_through, 2, 0) } else { (&mut *out, 0, light) };
                    let start = target.len();
                    emit(&empty, o.as_vec3(), r, u, away, layer, l, target);
                    emit(&full, o.as_vec3(), r, u, glam::Vec3::ZERO, layer, l, target);
                    if !through {
                        game.mark_entity(start, "minecraft:name_tag");
                    }
                }
            }
        }
        if t.armor || t.override_all {
            // `renderArmorPoints`.
            let armor = horizon_client::mobs::armor::values(m).0.ceil() as i64;
            if armor > 0 {
                let per_row = int("icons_per_row").max(1);
                let points = (armor as f32 / 2.0).ceil() as i64;
                let last_half = armor & 1 == 1;
                let max_x = (10.min(per_row) * 8 + 1) as f32 / 2.0;
                let extra = (((max + m.absorption) / 2.0 + per_row as f32 - 1.0) / per_row as f32) as i64;
                let mut row_lift = 0.0;
                for point in 0..10i64 {
                    if point % per_row == 0 {
                        row_lift = f64::from(scale) * 10.0 * ((point / 2 + per_row - 1) / per_row) as f64;
                    }
                    let o = pos
                        + glam::DVec3::new(
                            0.0,
                            f64::from(height) + 0.75 + f64::from(scale) * 10.0 * (extra - 1) as f64 + row_lift + lift_name,
                            0.0,
                        )
                        - camera
                        + up * offset;
                    let x = (point % per_row) as f32 * 8.0 - max_x;
                    let mut empty = HudBatch::new([1, 1]);
                    if let Some(rect) = sprite(&game.gui.atlas, "hud/armor_empty") {
                        empty.rect_uv(x, 0.0, 9.0, 9.0, rect, white);
                    }
                    let mut full = HudBatch::new([1, 1]);
                    if point < points {
                        let name = if point == points - 1 && last_half { "hud/armor_half" } else { "hud/armor_full" };
                        if let Some(rect) = sprite(&game.gui.atlas, name) {
                            full.rect_uv(x, 0.0, 9.0, 9.0, rect, white);
                        }
                    }
                    let (target, layer, l) = if through { (&mut *see_through, 2, 0) } else { (&mut *out, 0, light) };
                    emit(&empty, o.as_vec3(), r, u, away, layer, l, target);
                    emit(&full, o.as_vec3(), r, u, glam::Vec3::ZERO, layer, l, target);
                }
            }
        }
    }
    // `renderNumber`: the health as text, in the number colour and opacity.
    let text_color = color("number_color");
    for (o, id, through) in numbers {
        let Some(m) = game.mobs.iter().find(|m| m.id == id) else {
            continue;
        };
        let text = Text::literal(health_text(m)).with_color(text_color);
        let lines = [(text, 0.0)];
        let patches = if through {
            game.name_tag_quads(o, &lines, opacity, false, 2, 0, scale, see_through)
        } else {
            let start = out.len();
            let p = game.name_tag_quads(o, &lines, opacity, false, 0, light, scale, out);
            game.mark_entity(start, "minecraft:name_tag");
            p
        };
        game.map_patches.extend(patches);
    }
}

// ---------------------------------------------------------------------------------------------
// HudRenderer (the monitored entity) and the damage direction indicator
// ---------------------------------------------------------------------------------------------

fn argb_to_batch(argb: u32) -> u32 {
    horizon_client::hud_layout::abgr(argb)
}

/// `drawTriangle`: filled a row at a time, as the original.
fn triangle(batch: &mut HudBatch, atlas: &HudAtlas, mut p: [(f32, f32); 3], color: u32) {
    p.sort_by(|a, b| a.1.total_cmp(&b.1));
    let [(x0, y0), (x1, y1), (x2, y2)] = p;
    if y0 == y2 {
        return;
    }
    let lerp = |xa: f32, ya: f32, xb: f32, yb: f32, y: f32| if ya == yb { xa } else { xa + (xb - xa) * (y - ya) / (yb - ya) };
    let mut y = y0.floor() as i32;
    while y <= y2.ceil() as i32 {
        let fy = y as f32;
        if fy >= y0 && fy <= y2 {
            let (mut a, mut b) = if fy <= y1 {
                (lerp(x0, y0, x1, y1, fy), lerp(x0, y0, x2, y2, fy))
            } else {
                (lerp(x1, y1, x2, y2, fy), lerp(x0, y0, x2, y2, fy))
            };
            if a > b {
                std::mem::swap(&mut a, &mut b);
            }
            let (l, rr) = (a as i32, b as i32 + 1);
            batch.fill(atlas, l as f32, fy, (rr - l) as f32, 1.0, color);
        }
        y += 1;
    }
}

fn quad(batch: &mut HudBatch, atlas: &HudAtlas, p: [(f32, f32); 4], color: u32) {
    triangle(batch, atlas, [p[0], p[1], p[2]], color);
    triangle(batch, atlas, [p[0], p[2], p[3]], color);
}

pub(crate) fn hud(game: &crate::game::Game<'_>, batch: &mut HudBatch) {
    if !game.native_on(ID) || !game.native_window() {
        return;
    }
    let atlas = &game.gui.atlas;
    let (monitored, since, attacker) = TRACKER.with(|t| {
        let t = t.borrow();
        (t.monitored, t.since_damage, t.attacker)
    });
    // `drawNumberHealthGUIIndicator` at (20, 20).
    if let Some(m) = monitored.and_then(|id| game.mobs.iter().find(|m| m.id == id)) {
        let name = m
            .custom_name_text
            .as_ref()
            .map(|t| t.plain())
            .or_else(|| player_name(game, m))
            .unwrap_or_else(|| m.kind.id.trim_start_matches("minecraft:").to_owned());
        let c = 0xFF00_0000 | color("number_color");
        let shadow = on("render_number_display_shadow");
        batch.text_styled(atlas, &Text::literal(name), 20.0, 20.0, argb_to_batch(c), shadow);
        batch.text_styled(atlas, &Text::literal(health_text(m)), 20.0, 30.0, argb_to_batch(c), shadow);
    }
    // `DamageDirectionIndicatorRenderer.render`, in single player only.
    if !on("enable_damage_direction_indicators") || !game.has_singleplayer_server() {
        return;
    }
    let (Some(since), Some(attacker), Some(p)) = (since, attacker, game.player.as_ref()) else {
        return;
    };
    let visible = int("damage_direction_indicators_visibility_time");
    if i64::from(since) > visible * 20 {
        return;
    }
    let Some(a) = game.mobs.iter().find(|m| m.id == attacker) else {
        return;
    };
    let wrap = |d: f32| (d + 180.0).rem_euclid(360.0) - 180.0;
    let to = wrap(((a.position.z - p.position.z).atan2(a.position.x - p.position.x).to_degrees() as f32) - 90.0);
    let angle = wrap(to - game.yaw).to_radians();
    let mut alpha = 255i64;
    if on("damage_direction_indicators_fade_out") {
        let fade_time = int("damage_direction_indicators_fade_out_time");
        let delay = (visible - fade_time) * 20;
        if i64::from(since) >= delay {
            let progress = ((i64::from(since) - delay) as f32 / (fade_time * 20) as f32).min(1.0);
            alpha = 255 - (255.0 * progress) as i64;
        }
    }
    let alpha = alpha.clamp(0, 255) as u32;
    let s = int("damage_direction_indicators_scale") as f32;
    let rgb = color("damage_direction_indicators_color");
    let (cr, cg, cb) = (rgb >> 16 & 255, rgb >> 8 & 255, rgb & 255);
    let main = argb_to_batch(alpha << 24 | rgb);
    let dark = argb_to_batch(((alpha as f32 * 0.25) as u32) << 24 | (cr * 2 / 3) << 16 | (cg * 2 / 3) << 8 | cb * 2 / 3);
    let (cx, cy) = ((batch.width as i32 / 2) as f32, (batch.height as i32 / 2) as f32);
    let (radius, span, thickness) = (28.0 * s, 50.0f32, 1.6 * s);
    let point = |angle: f32, r: f32| (cx + r * angle.sin(), cy - r * angle.cos());
    for (inner, outer, c) in [(radius, radius + thickness, main), (radius + thickness, radius + thickness + s, dark)] {
        for i in 0..20 {
            let (t1, t2) = (i as f32 / 20.0, (i + 1) as f32 / 20.0);
            let a1 = angle - (span / 2.0).to_radians() + (span * t1).to_radians();
            let a2 = angle - (span / 2.0).to_radians() + (span * t2).to_radians();
            quad(batch, atlas, [point(a1, inner), point(a2, inner), point(a2, outer), point(a1, outer)], c);
        }
    }
    let (length, width) = (6.0 * s, 4.5 * s);
    let base = point(angle, radius);
    let tip = point(angle, radius + length);
    let perp = angle + std::f32::consts::FRAC_PI_2;
    let left = (base.0 + width * perp.sin(), base.1 - width * perp.cos());
    let right = (base.0 - width * perp.sin(), base.1 + width * perp.cos());
    triangle(batch, atlas, [tip, left, right], main);
}

// ---------------------------------------------------------------------------------------------
// ModCommands
// ---------------------------------------------------------------------------------------------

pub(crate) fn command(game: &mut crate::game::Game<'_>, command: &str) -> bool {
    let mut words = command.split_whitespace();
    if words.next() != Some("healthindicators") || !game.native_on(ID) {
        return false;
    }
    let rest: Vec<&str> = words.collect();
    let unquote = |s: &str| s.trim_matches('"').to_owned();
    match rest.as_slice() {
        [] => open_settings(game),
        _ if !on("enable_commands") => return false,
        ["offset", v] => {
            let Ok(v) = v.parse::<f64>() else {
                return false;
            };
            set("display_offset", format!("{v:?}"));
            message(game, &format!("Set heart offset to {}", (v * 100.0).floor() / 100.0), None);
        }
        ["indicator-type", v] => {
            let kind = match unquote(v).as_str() {
                "heart" => "HEARTS",
                "number" => "NUMBER",
                _ => {
                    message(game, "Unknown argument, please try again.", None);
                    return true;
                }
            };
            set("indicator_type", kind.to_owned());
            message(game, &format!("Set display type to {kind}"), None);
        }
        ["monitor", name @ ..] => {
            let name = unquote(&name.join(" "));
            let found = game.mobs.iter().find(|m| {
                m.custom_name_text.as_ref().is_some_and(|t| t.plain() == name)
                    || (is_player(m) && player_name(game, m).as_deref() == Some(name.as_str()))
            });
            match found.map(|m| m.id) {
                Some(id) => {
                    message(game, &format!("Now monitoring {name}"), None);
                    TRACKER.with(|t| t.borrow_mut().monitored = Some(id));
                }
                None => message(
                    game,
                    &format!("There is no entity named {name} in the world. It may have died or gone out of render distance."),
                    None,
                ),
            }
        }
        ["stop-monitoring"] => {
            TRACKER.with(|t| t.borrow_mut().monitored = None);
            message(game, "Stopped monitoring ", None);
        }
        _ => return false,
    }
    true
}

// ---------------------------------------------------------------------------------------------
// The settings (YACL's screen) on the mod's settings page
// ---------------------------------------------------------------------------------------------

fn shown_value(e: &Entry, v: &str) -> String {
    match e.kind {
        Kind::Bool(t, f) => (if v == "true" { t } else { f }).to_owned(),
        Kind::Enum(names) => names.iter().find(|(n, _)| *n == v).map_or(v.to_owned(), |(_, d)| (*d).to_owned()),
        Kind::Float(_, _, _, places) => format!("{:.*}", places as usize, v.parse::<f64>().unwrap_or(0.0)),
        Kind::Color => COLORS
            .iter()
            .find(|(hex, _)| hex.eq_ignore_ascii_case(v))
            .map_or(v.to_owned(), |(_, n)| (*n).to_owned()),
        Kind::List => {
            let n = v.split(',').filter(|s| !s.trim().is_empty()).count();
            format!("{n} entr{}", if n == 1 { "y" } else { "ies" })
        }
        Kind::Int(..) => v.to_owned(),
    }
}

fn section_entries(s: usize) -> impl Iterator<Item = &'static Entry> {
    ENTRIES.iter().filter(move |e| e.section == s)
}

pub(crate) fn settings(buttons: &mut Vec<String>) {
    match SECTION.get() {
        None => buttons.extend(SECTIONS.iter().map(|s| format!("{s}..."))),
        Some(s) => {
            for e in section_entries(s) {
                buttons.push(format!("{}: {}", e.caption, shown_value(e, &get(e.key))));
            }
            buttons.extend(["Reset".to_owned(), "Done".to_owned()]);
        }
    }
}

fn step(e: &Entry, back: bool, many: bool) -> Option<String> {
    let v = get(e.key);
    let n = if many { 10 } else { 1 };
    Some(match e.kind {
        Kind::Bool(..) => (if v == "true" { "false" } else { "true" }).to_owned(),
        Kind::Enum(names) => {
            let i = names.iter().position(|(x, _)| *x == v).unwrap_or(0);
            let len = names.len();
            names[if back { (i + len - 1) % len } else { (i + 1) % len }].0.to_owned()
        }
        Kind::Int(min, max, step) => {
            let x = v.parse::<i64>().unwrap_or(min) + if back { -step * n } else { step * n };
            (if x > max { min } else if x < min { max } else { x }).to_string()
        }
        Kind::Float(min, max, step, _) => {
            let x = v.parse::<f64>().unwrap_or(min) + if back { -step } else { step } * n as f64;
            let x = if x > max + 1e-9 { min } else if x < min - 1e-9 { max } else { x };
            format!("{:?}", (x * 1000.0).round() / 1000.0)
        }
        Kind::Color => {
            let i = COLORS.iter().position(|(hex, _)| hex.eq_ignore_ascii_case(&v));
            let len = COLORS.len();
            let next = match i {
                Some(i) if back => (i + len - 1) % len,
                Some(i) => (i + 1) % len,
                None => 0,
            };
            COLORS[next].0.to_owned()
        }
        // Edited in the config file, as a list of ids and player names.
        Kind::List => {
            if let Some(p) = path() {
                let _ = std::process::Command::new("notepad.exe").arg(&p).spawn();
            }
            return None;
        }
    })
}

pub(crate) fn click(game: &mut crate::game::Game<'_>, button: usize) {
    let back = game.modifiers.shift_key();
    let many = game.modifiers.control_key();
    let Some(s) = SECTION.get() else {
        if button < SECTIONS.len() {
            SECTION.set(Some(button));
        }
        return;
    };
    let entries: Vec<&Entry> = section_entries(s).collect();
    match button.checked_sub(entries.len()) {
        None => {
            if let Some(v) = step(entries[button], back, many) {
                set(entries[button].key, v);
            }
        }
        Some(0) => {
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
