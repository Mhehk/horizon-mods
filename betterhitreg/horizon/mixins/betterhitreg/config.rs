// BetterHitreg — mhv2 for Horizon (mihehk), a port of BetterHitreg by Jass
// (Apache-2.0): its settings file (`config/hitreg.properties`, the same keys,
// categories and defaults), `/hitreg`, the menu key (H) and the menu, on the
// mod's Options > Mods page, every row of the original.
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

/// `Toggle`: key, label, default.
pub(crate) const TOGGLES: [(&str, &str, bool); 34] = [
    ("toggle", "custom hitreg", true),
    ("safeRegsOnly", "safe regs only", true),
    ("ignoreShieldHolders", "ignore shield holders", false),
    ("debugInputs", "debug inputs", false),
    ("debugSounds", "debug sounds", false),
    ("alertDelays", "alert delays", false),
    ("alertGhosts", "alert ghosts", false),
    ("alertInconsistencies", "alert inconsistencies", false),
    ("alertFights", "alert fights", false),
    ("AlertJumpResets", "alert jump resets", false),
    ("legacySounds", "1.8 sounds", false),
    ("hideAnimations", "hide animations", false),
    ("hideArmor", "hide armor", false),
    ("hideAllParticles", "hide crit particles", false),
    ("hideOtherParticles", "only crit/sweep particles", false),
    ("particlesEveryHit", "particles on every hit", false),
    ("silenceOtherFights", "silence other fights", false),
    ("silenceSelf", "silence your hits", false),
    ("silenceThem", "silence their hits", false),
    ("silenceNonHits", "silence non-hits", false),
    ("hideOtherFights", "hide other fights", false),
    ("renderHitbox", "render target hitbox", false),
    ("renderCross", "render target cross", false),
    ("invisHitboxes", "show invisible hitboxes", false),
    ("RenderServerHitbox", "render server hitbox", false),
    ("RenderApproachHitbox", "render approach hitbox", false),
    ("RenderYourReach", "render your reach", false),
    ("RenderTheirReach", "render their reach", false),
    ("RenderYourJump", "render your jump range", false),
    ("RenderTheirJump", "render their jump range", false),
    ("PerfectHitColor", "color first tick hits", false),
    ("JumpResetColor", "color jump resets", false),
    ("VoidWorld", "void world", false),
    ("CustomGround", "custom ground", false),
];

/// `Setting`: key, category, default.
const SETTINGS: [(&str, &str, &str); 16] = [
    ("tutorial", "toggle", "true"),
    ("hitreg", "configure", "0"),
    ("total_fights", "tracked", "0"),
    ("fight_playtime_(seconds)", "tracked", "0"),
    ("muffle_amount", "configure", "0"),
    ("sharpen_amount", "configure", "0"),
    ("metronome", "configure", "0"),
    ("floor_grid_size", "configure", "0"),
    ("floor_grid_range", "configure", "16"),
    ("ground_height", "configure", "0.01"),
    ("ground_size", "configure", "512"),
    ("score_x", "configure", "1"),
    ("score_y", "configure", "1"),
    ("approach_hitbox_range", "configure", "6"),
    ("sound_hit_window", "configure", "150"),
    ("sound_recency_threshold", "configure", "50"),
];

/// `Style`: name, colour, opacity, category.
const STYLES: [(&str, &str, i32, &str); 26] = [
    ("cross_far", "FFFFFF", 255, "render"),
    ("cross_near", "FF0000", 255, "render"),
    ("cross_far_with_hitbox", "0000FF", 255, "render"),
    ("cross_near_with_hitbox", "0000FF", 255, "render"),
    ("hitbox_far", "FFFFFF", 255, "render"),
    ("hitbox_near", "FF0000", 255, "render"),
    ("server_hitbox_far", "7F00FF", 125, "render"),
    ("server_hitbox_near", "7F00FF", 125, "render"),
    ("approach_hitbox", "FFFFFF", 125, "render"),
    ("your_reach_far", "FFFFFF", 255, "render"),
    ("your_reach_near", "FF0000", 255, "render"),
    ("their_reach_far", "FFFFFF", 255, "render"),
    ("their_reach_near", "FF0000", 255, "render"),
    ("your_jump_far", "007FFF", 255, "render"),
    ("your_jump_near", "007FFF", 255, "render"),
    ("their_jump_far", "007FFF", 255, "render"),
    ("their_jump_near", "007FFF", 255, "render"),
    ("jump_reset", "FFFF00", 255, "render"),
    ("perfect_hit", "00FF00", 255, "render"),
    ("grid", "FFFFFF", 255, "render"),
    ("ground", "101010", 255, "render"),
    ("background", "000000", 230, "ui"),
    ("border", "646464", 255, "ui"),
    ("text", "DEDEDE", 255, "ui"),
    ("hovered", "FFF3A6", 255, "ui"),
    ("highlighted", "FFE350", 255, "ui"),
];

struct Props {
    values: BTreeMap<String, String>,
    categories: BTreeMap<String, &'static str>,
    stamp: Option<SystemTime>,
    checked: Instant,
}

thread_local! {
    static PROPS: RefCell<Option<Props>> = const { RefCell::new(None) };
    /// The menu's open section (None: the section list).
    static SECTION: Cell<Option<usize>> = const { Cell::new(None) };
    static TUTORIAL_SEEN: Cell<bool> = const { Cell::new(false) };
    static TICK: Cell<u64> = const { Cell::new(0) };
}

fn path() -> Option<PathBuf> {
    crate::game::Game::native_config_dir()
        .ok()
        .map(|d| d.join("hitreg.properties"))
}

fn stamp() -> Option<SystemTime> {
    std::fs::metadata(path()?).and_then(|m| m.modified()).ok()
}

fn defaults() -> (BTreeMap<String, String>, BTreeMap<String, &'static str>) {
    let mut values = BTreeMap::new();
    let mut categories = BTreeMap::new();
    for (key, _, default) in TOGGLES {
        values.insert(key.to_owned(), default.to_string());
        categories.insert(key.to_owned(), "toggle");
    }
    for (key, category, default) in SETTINGS {
        values.insert(key.to_owned(), default.to_owned());
        categories.insert(key.to_owned(), category);
    }
    for (name, hex, opacity, category) in STYLES {
        values.insert(format!("{name}_color"), hex.to_owned());
        categories.insert(format!("{name}_color"), category);
        values.insert(format!("{name}_opacity"), opacity.to_string());
        categories.insert(format!("{name}_opacity"), category);
    }
    (values, categories)
}

/// `Properties.load`: `key=value` lines, `#` and `!` comments.
fn read(values: &mut BTreeMap<String, String>) -> bool {
    let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) else {
        return false;
    };
    for line in text.lines() {
        let line = line.trim_start();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }
        let split = line.find(['=', ':']);
        let (k, v) = match split {
            Some(i) => (&line[..i], &line[i + 1..]),
            None => (line, ""),
        };
        values.insert(k.trim().replace("\\", ""), v.trim_start().to_owned());
    }
    true
}

/// `Settings.save`: "#Hitreg Settings", then each category's keys sorted.
fn write(p: &Props) {
    let Some(path) = path() else {
        return;
    };
    let mut out = String::from("#Hitreg Settings\n\n");
    for (title, category) in [
        ("Configure", "configure"),
        ("Render", "render"),
        ("UI", "ui"),
        ("Tracked", "tracked"),
        ("Toggle", "toggle"),
    ] {
        let mut keys: Vec<&String> = p
            .values
            .keys()
            .filter(|k| p.categories.get(*k).copied().unwrap_or("other") == category)
            .collect();
        if keys.is_empty() {
            continue;
        }
        keys.sort_by_key(|k| k.to_lowercase());
        out.push_str(&format!("#{title}\n"));
        for k in keys {
            out.push_str(&format!("{k}={}\n", p.values[k]));
        }
        out.push('\n');
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(&path, out) {
        log::error!(target: "horizon_betterhitreg", "Couldn't save file: {e}");
    }
}

fn with_props<R>(f: impl FnOnce(&mut Props) -> R) -> R {
    PROPS.with(|cell| {
        let mut cell = cell.borrow_mut();
        let p = cell.get_or_insert_with(|| {
            let (mut values, categories) = defaults();
            let found = read(&mut values);
            let mut p = Props { values, categories, stamp: None, checked: Instant::now() };
            if !found {
                write(&p);
            }
            p.stamp = stamp();
            p
        });
        f(p)
    })
}

pub(crate) fn get(key: &str) -> String {
    with_props(|p| p.values.get(key).cloned()).unwrap_or_default()
}

pub(crate) fn on(key: &str) -> bool {
    get(key).trim() == "true"
}

pub(crate) fn int(key: &str) -> i64 {
    let v = get(key);
    v.trim().parse::<i64>().unwrap_or_else(|_| v.trim().parse::<f64>().map_or(0, |f| f as i64))
}

/// For the fight statistics (`Settings.addFight`).
pub(crate) fn set_value(key: &str, value: String) {
    set(key, value);
}

/// A style's colour and opacity as ARGB (`Style.argb`).
pub(crate) fn style_argb(name: &str) -> u32 {
    let hex = get(&format!("{name}_color"));
    let rgb = u32::from_str_radix(hex.trim().trim_start_matches('#'), 16).unwrap_or(0xFF_FFFF) & 0xFF_FFFF;
    let alpha = int(&format!("{name}_opacity")).clamp(0, 255) as u32;
    alpha << 24 | rgb
}

fn set(key: &str, value: String) {
    with_props(|p| {
        p.values.insert(key.to_owned(), value);
        write(p);
        p.stamp = stamp();
    });
}

fn watch(now: Instant) {
    with_props(|p| {
        if now.duration_since(p.checked) < Duration::from_secs(1) {
            return;
        }
        p.checked = now;
        let s = stamp();
        if s != p.stamp {
            p.stamp = s;
            let (mut values, _) = defaults();
            read(&mut values);
            p.values = values;
        }
    });
}

// ---------------------------------------------------------------------------------------------
// Messages and toggles (MultiVersion.message, Toggle.toggle, Commands)
// ---------------------------------------------------------------------------------------------

/// `MultiVersion.message`: "Hitreg | " then the line, with its colour codes.
pub(crate) fn message(game: &mut crate::game::Game<'_>, text: &str) {
    let focused = game.screen == crate::game::Screen::Chat;
    // Gold (0xFFD700), as `MultiVersion.message` styles the line.
    let line = horizon_client::text::Text::legacy(&format!("Hitreg §8|§r {text}")).with_color(0xFFD700);
    let ticks = game.visual_ticks();
    let atlas = &game.gui.atlas;
    game.ui
        .chat
        .add(atlas, line, None, horizon_client::chat::ChatSource::Client, ticks, focused);
}

fn on_or_off(v: bool) -> &'static str {
    if v { "§aon§7" } else { "§coff§7" }
}

/// `Toggle.toggle`.
fn toggle(game: &mut crate::game::Game<'_>, key: &str) {
    let Some((key, label, _)) = TOGGLES.iter().find(|t| t.0 == key) else {
        return;
    };
    let value = !on(key);
    set(key, value.to_string());
    message(game, &format!("{label} §7is now {}", on_or_off(value)));
    let not = |v: bool| if v { "will no longer" } else { "will now" };
    match *key {
        "safeRegsOnly" => message(game, &format!("§7first hits {} use custom hitreg", not(value))),
        "ignoreShieldHolders" => message(
            game,
            &format!("§7players with a shield (blocking or not) {} be affected by custom hitreg", not(value)),
        ),
        "renderHitbox" | "renderCross" | "RenderServerHitbox" | "RenderApproachHitbox" | "RenderYourReach"
        | "RenderTheirReach" | "RenderYourJump" | "RenderTheirJump" | "PerfectHitColor" | "JumpResetColor"
        | "CustomGround" => message(game, "§7colors can be edited via §f/hitreg color <key> <hex> <opacity>"),
        "debugSounds" => message(
            game,
            &format!(
                "§7every hit sound near your fight {} be logged to latest.log, possible doubles also show in chat",
                if value { "will now" } else { "will no longer" }
            ),
        ),
        "VoidWorld" => message(game, "§7this feature may not work if you're using render mods or a modded client"),
        "invisHitboxes" => message(
            game,
            &format!(
                "§7invisible players' hitboxes {} show with §fF3+B§7 and target hitbox",
                if value { "will now" } else { "will no longer" }
            ),
        ),
        _ => {}
    }
}

/// `/hitreg color <style>` shows a style, `/hitreg color <style> <hex> [opacity]` sets it.
fn color(game: &mut crate::game::Game<'_>, style: &str, hex: Option<&str>, opacity: Option<&str>) {
    let Some((name, ..)) = STYLES.iter().find(|s| s.0 == style) else {
        return;
    };
    let upper = name.to_uppercase();
    let (hex_key, opacity_key) = (format!("{name}_color"), format!("{name}_opacity"));
    let (old_hex, old_opacity) = (get(&hex_key), int(&opacity_key));
    let Some(new_hex) = hex else {
        message(game, &format!("{upper} current color: #{old_hex} opacity: {old_opacity}"));
        return;
    };
    if new_hex.len() != 6 || !new_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        message(game, "Invalid hexadecimal color");
        return;
    }
    let new_hex = new_hex.to_uppercase();
    let new_opacity = opacity.and_then(|o| o.parse::<i64>().ok()).unwrap_or(255).clamp(0, 255);
    set(&hex_key, new_hex.clone());
    set(&opacity_key, new_opacity.to_string());
    message(
        game,
        &format!("Changed {upper} from #{old_hex} {old_opacity} to #{new_hex} {new_opacity}"),
    );
}

/// `Commands.setSetting` for `hitreg` and `metronome`.
fn set_setting(game: &mut crate::game::Game<'_>, key: &str, value: i64) {
    let mut extra = String::new();
    if key == "hitreg" && !on("toggle") {
        message(game, "custom hitreg §7is currently off, use §f/hitreg toggle §7to enable it");
    } else if key == "metronome" {
        extra = format!(" §7ticks ({}ms)", value * 50);
    }
    set(key, value.to_string());
    let name = key.replace(['_', '-'], " ");
    // `Setting.isDisabled`: hitreg's and the metronome's disabled values.
    let disabled = (key == "hitreg" && value == 0) || (key == "metronome" && value == 9);
    if disabled {
        message(game, &format!("{name} §cdisabled"));
    } else {
        message(game, &format!("{name} §7set to §f{value}{extra}"));
    }
}

fn open_menu(game: &mut crate::game::Game<'_>) {
    // The tutorial ends once the menu has been opened (UIScreen).
    if on("tutorial") {
        set("tutorial", "false".to_owned());
    }
    game.list_native_mods();
    if let Some(i) = game.settings.mods.installed.iter().position(|m| m.id == super::betterhitreg::ID) {
        SECTION.set(None);
        game.open_options_at(false, horizon_client::options_ui::Page::NativeMod(i));
    }
}

/// `/hitreg`, `/hitreg <toggle>`, `/hitreg setHitreg [ms]`, `/hitreg setMetronome [ticks]`.
pub(crate) fn command(game: &mut crate::game::Game<'_>, command: &str) -> bool {
    let mut words = command.split_whitespace();
    if words.next() != Some("hitreg") || !game.native_on(super::betterhitreg::ID) {
        return false;
    }
    match (words.next(), words.next()) {
        (None, _) => open_menu(game),
        (Some("color"), Some(style)) => {
            let (hex, opacity) = (words.next(), words.next());
            color(game, style, hex, opacity);
        }
        (Some("setHitreg"), v) => set_setting(game, "hitreg", v.and_then(|v| v.parse().ok()).unwrap_or(0)),
        (Some("setMetronome"), v) => set_setting(game, "metronome", v.and_then(|v| v.parse().ok()).unwrap_or(0)),
        (Some(word), _) if TOGGLES.iter().any(|t| t.0.eq_ignore_ascii_case(word)) => {
            let key = TOGGLES.iter().find(|t| t.0.eq_ignore_ascii_case(word)).map_or(word, |t| t.0);
            toggle(game, key);
        }
        _ => return false,
    }
    true
}

// ---------------------------------------------------------------------------------------------
// The menu (UIScreen) on the mod's settings page
// ---------------------------------------------------------------------------------------------

enum Row {
    Toggle(&'static str, &'static str),
    Hitreg,
    Metronome,
    /// `muffle_amount` / `sharpen_amount`, 0 to 100 %.
    Percent(&'static str, &'static str),
    GridFloor,
}

/// `UIScreen`'s sections and rows, in its order.
const SECTIONS: [(&str, &[Row]); 4] = [
    ("Hitreg", &[Row::Toggle("toggle", "Hitreg"), Row::Hitreg]),
    (
        "Utility",
        &[
            Row::Toggle("safeRegsOnly", "Safe Regs Only"),
            Row::Toggle("ignoreShieldHolders", "Ignore Shield Holders"),
            Row::Toggle("debugInputs", "Debug Inputs"),
            Row::Toggle("alertDelays", "Alert Delays"),
            Row::Toggle("alertGhosts", "Alert Ghosts"),
            Row::Toggle("alertInconsistencies", "Alert Misplaces"),
            Row::Toggle("alertFights", "Alert Fight Statistics"),
            Row::Toggle("AlertJumpResets", "Alert Jump Resets"),
        ],
    ),
    (
        "Audio",
        &[
            Row::Toggle("silenceOtherFights", "Mute Other Fights"),
            Row::Toggle("legacySounds", "1.8 Hit Sounds"),
            Row::Toggle("silenceNonHits", "Mute Non-hit Sounds"),
            Row::Toggle("silenceSelf", "Mute Hits: You"),
            Row::Toggle("silenceThem", "Mute Hits: Them"),
            Row::Percent("muffle_amount", "Hit Muffling"),
            Row::Percent("sharpen_amount", "Hit Sharpening"),
            Row::Metronome,
        ],
    ),
    (
        "Render",
        &[
            Row::Toggle("hideOtherFights", "Hide Other Fights"),
            Row::Toggle("hideAnimations", "Hide Animations"),
            Row::Toggle("hideArmor", "Hide Armor"),
            Row::Toggle("hideAllParticles", "Hide All Particles"),
            Row::Toggle("hideOtherParticles", "Hide Other Particles"),
            Row::Toggle("particlesEveryHit", "Always Hit Particles"),
            Row::Toggle("renderHitbox", "Show Target Hitbox"),
            Row::Toggle("renderCross", "Show Target Cross"),
            Row::Toggle("RenderServerHitbox", "Show Server Hitbox"),
            Row::Toggle("RenderApproachHitbox", "Show Approach Hitbox"),
            Row::Toggle("PerfectHitColor", "Color Perfect Hits"),
            Row::Toggle("JumpResetColor", "Color Jump Resets"),
            Row::Toggle("RenderYourReach", "Hit Range: You"),
            Row::Toggle("RenderTheirReach", "Hit Range: Them"),
            Row::Toggle("RenderYourJump", "Jump Range: You"),
            Row::Toggle("RenderTheirJump", "Jump Range: Them"),
            Row::Toggle("VoidWorld", "Void World"),
            Row::Toggle("CustomGround", "Custom Ground"),
            Row::GridFloor,
            Row::Toggle("invisHitboxes", "Show Invis Hitboxes"),
        ],
    ),
];

/// The options Horizon's renderer and sound engine leave no way to carry out.
const UNSUPPORTED: [&str; 3] = ["VoidWorld", "muffle_amount", "sharpen_amount"];

pub(crate) fn float(key: &str) -> f64 {
    get(key).trim().parse::<f64>().unwrap_or(0.0)
}

fn row_label(row: &Row) -> String {
    let (avg, ghosts, misplaces) = super::betterhitreg_fight::last100();
    let stat = |v: Option<i64>, unit: &str| v.map_or_else(String::new, |v| format!(" ({v}{unit})"));
    let mark = |key: &str| if UNSUPPORTED.contains(&key) { " (Horizon: yok)" } else { "" };
    match row {
        Row::Toggle(key, label) => {
            let extra = match *key {
                "alertDelays" => stat(avg, "ms"),
                "alertGhosts" => stat(ghosts, "%"),
                "alertInconsistencies" => stat(misplaces, "%"),
                _ => String::new(),
            };
            format!("{label}{extra}: {}{}", if on(key) { "ON" } else { "OFF" }, mark(key))
        }
        Row::Hitreg => format!("Hitreg: {}ms", int("hitreg")),
        Row::Metronome => {
            let t = int("metronome");
            if t < 10 { "Metronome: OFF".to_owned() } else { format!("Metronome: {t}t") }
        }
        Row::Percent(key, label) => {
            let v = (float(key).clamp(0.0, 1.0) * 100.0).round() as i64;
            format!("{label}: {}{}", if v == 0 { "OFF".to_owned() } else { format!("{v}%") }, mark(key))
        }
        Row::GridFloor => {
            let v = int("floor_grid_size");
            if v <= 0 { "Grid Floor: OFF".to_owned() } else { format!("Grid Floor: {v}") }
        }
    }
}

pub(crate) fn settings(buttons: &mut Vec<String>) {
    match SECTION.get() {
        None => buttons.extend(SECTIONS.iter().map(|(name, _)| format!("{name}..."))),
        Some(s) => {
            buttons.extend(SECTIONS[s].1.iter().map(row_label));
            buttons.push("Done".to_owned());
        }
    }
}

pub(crate) fn click(game: &mut crate::game::Game<'_>, button: usize) {
    let back = game.modifiers.shift_key();
    let many = game.modifiers.control_key();
    let Some(s) = SECTION.get() else {
        if button < SECTIONS.len() {
            // The tutorial ends once the menu has been opened (UIScreen).
            if on("tutorial") {
                set("tutorial", "false".to_owned());
            }
            SECTION.set(Some(button));
        }
        return;
    };
    let rows = SECTIONS[s].1;
    let Some(row) = rows.get(button) else {
        SECTION.set(None);
        return;
    };
    match row {
        Row::Toggle(key, _) => {
            toggle(game, key);
            if *key == "toggle" && !on("toggle") {
                message(game, "custom hitreg §7is currently off, check the box to enable it");
            }
        }
        // The Hitreg slider: 0 to 200 ms.
        Row::Hitreg => {
            let d = if many { 50 } else { 5 };
            let v = int("hitreg") + if back { -d } else { d };
            let v = if v > 200 { 0 } else if v < 0 { 200 } else { v };
            set_setting(game, "hitreg", v);
        }
        // The Metronome slider: 9 (off) to 25 ticks.
        Row::Metronome => {
            let v = int("metronome").max(9) + if back { -1 } else { 1 };
            let v = if v > 25 { 9 } else if v < 9 { 25 } else { v };
            set_setting(game, "metronome", v);
        }
        // The muffling and sharpening sliders: 0 to 100 % in fives.
        Row::Percent(key, _) => {
            let d = if many { 25 } else { 5 };
            let v = (float(key) * 100.0).round() as i64 + if back { -d } else { d };
            let v = if v > 100 { 0 } else if v < 0 { 100 } else { v };
            set(key, format!("{}", v as f64 / 100.0));
        }
        // The Grid Floor slider: 0 (off) to 16 blocks.
        Row::GridFloor => {
            let d = if many { 4 } else { 1 };
            let v = int("floor_grid_size") + if back { -d } else { d };
            let v = if v > 16 { 0 } else if v < 0 { 16 } else { v };
            set("floor_grid_size", v.to_string());
        }
    }
}


/// Each client tick: the metronome, the file read again when edited, the
/// tutorial's lines, and the settings page's section reset when left.
pub(crate) fn tick(game: &mut crate::game::Game<'_>) {
    use horizon_client::options_ui::Page;
    // A single player world's server runs a `Game` of its own: the window's only.
    if !game.native_window() {
        return;
    }
    watch(Instant::now());
    let on_page = game.screen == crate::game::Screen::Options
        && matches!(game.ui.options.page(), Page::NativeMod(i)
            if game.settings.mods.installed.get(i).is_some_and(|m| m.id == super::betterhitreg::ID));
    if !on_page {
        SECTION.set(None);
    }
    if !game.native_on(super::betterhitreg::ID) || game.player.is_none() {
        return;
    }
    let tick = TICK.get() + 1;
    TICK.set(tick);
    let metronome = int("metronome");
    if metronome >= 10 && tick % metronome as u64 == 0 {
        game.audio.play("minecraft:ui.button.click", None, 1.0, 1.0);
    }
    if on("tutorial") && !TUTORIAL_SEEN.get() {
        TUTORIAL_SEEN.set(true);
        let key = game
            .native_mod_key(super::betterhitreg::ID, "menu", "key.keyboard.h")
            .replace("key.keyboard.", "")
            .replace("key.mouse.", "")
            .replace('.', " ")
            .to_uppercase();
        message(game, "Thanks for using BetterHitreg!");
        message(game, &format!("use /hitreg or press {key} to configure"));
        message(game, "(you can click on these messages)");
    }
}

/// The menu key (Open Hitreg Menu, H) with no screen open.
pub(crate) fn key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    if !pressed || !game.native_on(super::betterhitreg::ID) || !game.native_window() || !game.native_playing() {
        return;
    }
    if key == game.native_mod_key(super::betterhitreg::ID, "menu", "key.keyboard.h") {
        open_menu(game);
    }
}
