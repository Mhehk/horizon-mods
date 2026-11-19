// ShulkerBoxTooltip — Horizon port (mhv2) by mihehk: the configuration screen, a port of
// ShulkerBoxTooltip's impl/config/gui (ConfigScreen, ConfigCategoryTab, ConfigEntryList and the
// entry classes) by MisterPeModder, MIT licence (see the core module's header).
//
// Opened from Options > Mods (Mod Menu's config button); drawn as the mod's own screen. Values
// are edited as their setting strings and written by "Save & Quit".
use horizon_client::hud::HudBatch;
use horizon_client::text::Text;
use horizon_client::ui;
use std::cell::RefCell;
use std::collections::HashMap;

use super::shulkerboxtooltip as core;

const TAB_H: f32 = 24.0;
const FOOTER: f32 = 33.0;
const ROW_H: f32 = 24.0;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Bool,
    Enum(&'static [(&'static str, &'static str)]),
    Int,
    Color,
    Key,
    /// CategoryTitleConfigEntry.
    Title,
    /// PrefixTextConfigEntry.
    Prefix,
}

#[derive(Clone)]
struct Entry {
    kind: Kind,
    /// The setting key (the Java field path), or the text of a title/prefix.
    key: String,
    title: String,
    tooltip: &'static str,
    restart: bool,
}

const MERGING: &[(&str, &str)] = &[("IGNORE", "Ignore"), ("FIRST_ITEM", "First Item"), ("SEPARATE", "Separate Items")];
const ORDER: &[(&str, &str)] = &[("STACK_SIZE", "Stack Size"), ("PREVIEW_ORDER", "Preview Order")];
const THEME: &[(&str, &str)] = &[("SHULKERBOXTOOLTIP", "ShulkerBoxTooltip"), ("VANILLA", "Vanilla")];
const THEME_BUNDLE: &[(&str, &str)] = &[("VANILLA", "Vanilla"), ("VANILLA_PLUS", "Vanilla+"), ("SHULKERBOXTOOLTIP", "ShulkerBoxTooltip")];
const POSITION: &[(&str, &str)] = &[("INSIDE", "Inside"), ("OUTSIDE", "Outside"), ("OUTSIDE_TOP", "Outside (top)"), ("OUTSIDE_BOTTOM", "Outside (bottom)")];
const TOOLTIP_TYPE: &[(&str, &str)] = &[("VANILLA", "Vanilla"), ("MOD", "Modded"), ("NONE", "None")];
const LOOT: &[(&str, &str)] = &[("HIDE", "Hide"), ("SIMPLE", "Simple"), ("ADVANCED", "Advanced")];
const ENDER_SYNC: &[(&str, &str)] = &[("NONE", "None"), ("ACTIVE", "Active"), ("PASSIVE", "Passive")];

fn value(kind: Kind, key: &str, title: &str, tooltip: &'static str, restart: bool) -> Entry {
    Entry { kind, key: key.into(), title: title.into(), tooltip, restart }
}

/// "orange_shulker_box" → "Orange Shulker Box" (the block/item names en_us gives).
fn english(id: &str) -> String {
    id.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `RootConfigNode`: the categories in their ordinals, each field in declaration order.
fn tabs() -> Vec<(&'static str, Vec<Entry>)> {
    let mut colors = vec![value(Kind::Bool, "colors.coloredPreview", "Colored Preview Window", "Controls whether the preview window should be colored.", false)];
    for (cat, title) in [("default", "Miscellaneous"), ("shulker_boxes", "Shulker Boxes"), ("bundles", "Bundles")] {
        colors.push(Entry { kind: Kind::Title, key: title.into(), title: title.into(), tooltip: "", restart: false });
        for c in core::COLORS.iter().filter(|c| c.category == cat) {
            let name = match c.id {
                "default" => "Default".to_string(),
                "generic_container" => "Generic Container".to_string(),
                id => english(id),
            };
            colors.push(value(Kind::Color, &core::color_setting_key(c), &name, "", false));
        }
    }
    vec![
        ("Preview", vec![
            value(Kind::Bool, "preview.enable", "Enable Preview", "Toggles the shulker box preview.", false),
            value(Kind::Bool, "preview.swapModes", "Swap Modes", "Swaps the preview modes.\nIf true, pressing the preview key will show the full preview instead.", false),
            value(Kind::Bool, "preview.alwaysOn", "Always On", "If on, the preview is always displayed,\nregardless of the preview key being pressed.", false),
            value(Kind::Enum(MERGING), "preview.compactPreviewNbtBehavior", "Compact Preview Item Components", "In compact mode, how should items with the same ID\nbut different component data be compacted?\nIgnored:\n Ignores component data\nFirst Item:\n Items are displayed as all having\n the same component as the first item\nSeparate:\n Separates items with different component data", false),
            value(Kind::Enum(ORDER), "preview.compactPreviewOrder", "Compact Preview Item Order", "How compact preview items should be ordered.\nStack Size: sort stacks by size, largest first.\nPreview Order: preserve the order of items as they appear in the container preview.", false),
            value(Kind::Int, "preview.defaultMaxRowSize", "Maximum Row Size", "The max number of items in a row\nMay not affect modded containers.", false),
            value(Kind::Bool, "preview.serverIntegration", "Server Integration", "If on, the client will try to send packets to servers\nto allow extra preview information\nsuch as ender chest previews.", true),
            value(Kind::Enum(THEME), "preview.theme", "Theme", "The theme to use for preview windows.\nShulkerBoxTooltip:\n ShulkerBoxTooltip's default look and feel.\nVanilla:\n Mimics the style of vanilla bundle previews.", false),
            value(Kind::Enum(THEME_BUNDLE), "preview.themeBundle", "Theme (Bundle)", "The theme to use for bundle preview windows.\nVanilla:\n Fully disable ShulkerBoxTooltip's changes to bundles.\nVanilla+:\n Mimics vanilla theme, with enhancements like no size limit and preview locking.\nShulkerBoxTooltip:\n ShulkerBoxTooltip's custom look and feel.", false),
            value(Kind::Enum(POSITION), "preview.position", "Window Position", "The position of the preview window.\nInside:\n Inside the item's tooltip.\nOutside:\n Outside the item's tooltip, moves depending on the screen borders.\nOutside (top):\n Always at the top of the item's tooltip.\nOutside (bottom):\n Always at the bottom of the item's tooltip.", false),
            value(Kind::Bool, "preview.shortItemCounts", "Abbreviate large item counts", "If on, large item counts in compact previews will be shortened.", false),
            value(Kind::Bool, "preview.genericContainerPreview", "Generic Container Preview", "If on, items with the container data component\nthat are not explicitly supported will still show a preview,\nas if they were a shulker box.", false),
        ]),
        ("Tooltip", vec![
            value(Kind::Bool, "tooltip.showKeyHints", "Show Key Hints", "Controls whether the key hints in the container's tooltip should be displayed.", false),
            value(Kind::Enum(TOOLTIP_TYPE), "tooltip.type", "Tooltip Type", "The tooltip to use.\nVanilla:\n The vanilla tooltip (shows the first 5 items)\nModded:\n The mod's tooltip\nNone: No tooltip", false),
            value(Kind::Enum(LOOT), "tooltip.lootTableInfoType", "Loot Table Info Type", "Shows info about the current loot table of the item if present.\nVisible only when Tooltip Type is set to Modded.\nHide:\n No loot table info, default.\nSimple:\n Displays whether the stack uses a loot table.\nAdvanced:\n Shows the loot table used by the item.", false),
            value(Kind::Bool, "tooltip.hideShulkerBoxLore", "Hide Shulker Box Lore", "If on, the mod hides the custom text on shulker box tooltips.\nUse this option when a server-side preview datapack clashes with the mod.", false),
        ]),
        ("Colors", colors),
        ("Controls", vec![
            value(Kind::Key, "controls.previewKey", "Open Stack Preview", "Press this key when hovering a container stack\nto open the preview window", false),
            value(Kind::Key, "controls.fullPreviewKey", "Open Stack Full Preview", "Press this key when hovering a container stack\nto open the full preview window", false),
            value(Kind::Key, "controls.lockTooltipKey", "Lock Tooltip", "Hold this key when previewing a stack to lock the tooltip.", false),
        ]),
        ("Server", vec![
            Entry { kind: Kind::Prefix, key: "These options are for the servers to set. On multiplayer, changing these options from this menu will not affect the server's configuration.".into(), title: String::new(), tooltip: "", restart: false },
            value(Kind::Bool, "server.clientIntegration", "Client Integration", "If on, the server will be able to provide extra information\nabout containers to the clients with the mod installed.\nDisabling this option will disable all of the options below.", true),
            value(Kind::Enum(ENDER_SYNC), "server.enderChestSyncType", "Ender Chest Synchronization Type", "Changes the way the ender chest content preview is synchronized.\nNone:\n No synchronization,\n prevents clients from seeing a preview of their ender chest.\nActive:\n Ender chest contents are synchronized when changed.\nPassive:\n Ender chest contents are synchronized\n when the client opens a preview.", true),
        ]),
    ]
}

#[derive(Clone, Copy, PartialEq)]
enum Confirm {
    /// ConfirmScreen of onClose: "Changes Not Saved".
    Quit,
    /// ConfirmScreen of saveAndQuit: "Restart Required".
    Restart,
}

struct State {
    tabs: Vec<(&'static str, Vec<Entry>)>,
    tab: usize,
    scroll: Vec<f32>,
    /// The values being edited, the saved ones (`isActiveValue`) and the defaults.
    editing: HashMap<String, String>,
    active: HashMap<String, String>,
    defaults: HashMap<String, String>,
    /// The text fields of the integer and colour entries, by setting key.
    fields: HashMap<String, ui::EditBox>,
    focused: Option<String>,
    /// KeyValueConfigEntry waiting for a key (`selectedKeyNode`).
    selected_key: Option<String>,
    confirm: Option<Confirm>,
    size: [f32; 2],
    /// The Options > Mods page to go back to (`previous`).
    back_to: Option<usize>,
}

/// What a read-only hook asks the next frame to do.
#[derive(Clone, Copy)]
enum Pending {
    Close,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    static PENDING: RefCell<Option<Pending>> = const { RefCell::new(None) };
}

fn map(c: &core::Config) -> HashMap<String, String> {
    c.entries().into_iter().collect()
}

impl State {
    fn new(saved: &core::Config, back_to: Option<usize>) -> State {
        let tabs = tabs();
        let n = tabs.len();
        State {
            tabs,
            tab: 0,
            scroll: vec![0.0; n],
            editing: map(saved),
            active: map(saved),
            defaults: map(&core::Config::default()),
            fields: HashMap::new(),
            focused: None,
            selected_key: None,
            confirm: None,
            size: [0.0, 0.0],
            back_to,
        }
    }
    fn get(&self, key: &str) -> String {
        self.editing.get(key).cloned().unwrap_or_default()
    }
    /// `ValueConfigNode.validate`.
    fn error(&self, e: &Entry) -> Option<&'static str> {
        match e.kind {
            Kind::Int => match self.get(&e.key).trim().parse::<i32>() {
                Ok(v) if v > 0 => None,
                _ => Some("Must be greater than zero"),
            },
            Kind::Color => core::parse_color(&self.get(&e.key)).map_or(Some("Invalid color code"), |_| None),
            _ => None,
        }
    }
    fn changed(&self, e: &Entry) -> bool {
        self.editing.get(&e.key) != self.active.get(&e.key)
    }
    fn is_default(&self, e: &Entry) -> bool {
        self.editing.get(&e.key) == self.defaults.get(&e.key)
    }
    fn any_changed(&self) -> bool {
        self.editing != self.active
    }
    fn any_error(&self) -> bool {
        self.tabs.iter().flat_map(|(_, es)| es).any(|e| self.error(e).is_some())
    }
    fn tab_changed(&self, t: usize) -> bool {
        self.tabs[t].1.iter().any(|e| self.changed(e))
    }
    fn tab_error(&self, t: usize) -> bool {
        self.tabs[t].1.iter().any(|e| self.error(e).is_some())
    }
    fn set(&mut self, key: &str, v: String) {
        self.editing.insert(key.to_string(), v.clone());
        if let Some(f) = self.fields.get_mut(key)
            && f.value != v
        {
            f.set_value(&v);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Opening and closing
// ---------------------------------------------------------------------------------------------

/// The Mods page's button for this mod (Mod Menu's config button).
pub fn settings_buttons(buttons: &mut Vec<String>) {
    buttons.push("ShulkerBoxTooltip Configuration…".into());
}

/// The button: the Options screen gives way to the configuration screen.
pub fn open(game: &mut super::Game<'_>) {
    if game.title.is_some() {
        // A mod's own screen can't open over the title menus (no world).
        return;
    }
    let back_to = game.settings.mods.installed.iter().position(|m| m.id == core::ID);
    game.screen = super::Screen::Playing;
    if !game.native_mod_screen_open(core::ID) {
        return;
    }
    let saved = core::config(game);
    STATE.with(|s| *s.borrow_mut() = Some(State::new(&saved, back_to)));
}

/// Each frame (frame hook, which may write): a requested close, back to Options > Mods.
pub fn frame(game: &mut super::Game<'_>) {
    let open = game.native_mod_screen().is_some_and(|id| id == core::ID);
    let pending = PENDING.with(|p| p.borrow_mut().take());
    if !open {
        // Escape or another screen closed it.
        if STATE.with(|s| s.borrow().is_some()) && pending.is_none() {
            STATE.with(|s| *s.borrow_mut() = None);
        }
        if pending.is_none() {
            return;
        }
    }
    if let Some(Pending::Close) = pending {
        let back_to = STATE.with(|s| s.borrow_mut().take().and_then(|s| s.back_to));
        game.native_mod_screen_close(core::ID);
        if let Some(i) = back_to {
            game.open_options_at(false, horizon_client::options_ui::Page::NativeMod(i));
        }
    }
}

fn close() {
    PENDING.with(|p| *p.borrow_mut() = Some(Pending::Close));
}

/// `ConfigScreen.onClose`.
fn on_close(s: &mut State) {
    if !s.any_changed() {
        close();
    } else {
        s.confirm = Some(Confirm::Quit);
    }
}

/// `ConfigScreen.saveAndQuit`.
fn save_and_quit(game: &super::Game<'_>, s: &mut State) {
    let restart = s.tabs.iter().flat_map(|(_, es)| es).any(|e| e.restart && s.changed(e));
    for (k, v) in &s.editing {
        if s.active.get(k) != Some(v) {
            game.native_set_setting(core::ID, k, v);
        }
    }
    core::reload_config();
    s.active = s.editing.clone();
    if restart {
        s.confirm = Some(Confirm::Restart);
    } else {
        close();
    }
}

fn mine(game: &super::Game<'_>) -> bool {
    game.native_mod_screen().is_some_and(|id| id == core::ID)
}

// ---------------------------------------------------------------------------------------------
// Layout (HeaderAndFooterLayout(24, 33), TabNavigationBar, ConfigEntryList)
// ---------------------------------------------------------------------------------------------

/// TabNavigationBar: tabs of `roundToward(min(400, w) - 28) / n, 2)` centred.
fn tab_layout(w: f32, n: usize) -> (f32, f32) {
    let total = w.min(400.0) - 28.0;
    let tab = ((total / n as f32) / 2.0).round() * 2.0;
    (((w - tab * n as f32) / 2.0).floor(), tab)
}
struct Rows {
    top: f32,
    bottom: f32,
    left: f32,
    right: f32,
}
/// ConfigEntryList: row width `width - 80`, centred; rows 24 high from the list's top + 4.
fn rows(w: f32, h: f32) -> Rows {
    let width = w - 80.0;
    let left = (w / 2.0 - width / 2.0 + 2.0).floor();
    Rows { top: TAB_H, bottom: h - FOOTER, left, right: left + width - 4.0 }
}
fn content_height(n: usize) -> f32 {
    n as f32 * ROW_H + 4.0
}
fn max_scroll(s: &State, r: &Rows) -> f32 {
    (content_height(s.tabs[s.tab].1.len()) - (r.bottom - r.top)).max(0.0)
}
/// The value widgets of a row, right to left: undo, reset, then the value (ValueConfigEntry).
struct RowRects {
    undo: [f32; 4],
    reset: [f32; 4],
    value: [f32; 4],
    swatch: [f32; 4],
}
fn row_rects(kind: Kind, right: f32, y: f32) -> RowRects {
    let undo = [right - 20.0, y, 20.0, 20.0];
    let reset = [undo[0] - 2.0 - 20.0, y, 20.0, 20.0];
    let (value, swatch) = match kind {
        // IntegerValueConfigEntry: a 158 field less the two buttons, 3 left of reset, one lower.
        Kind::Int => {
            let w = 158.0 - 20.0 - 2.0 - 20.0 - 2.0;
            ([reset[0] - w - 3.0, y + 1.0, w, 18.0], [0.0; 4])
        }
        // ColorValueConfigEntry: a 138 field and the 18 px swatch left of it.
        Kind::Color => {
            let w = 138.0 - 20.0 - 2.0 - 20.0 - 2.0;
            let field = [reset[0] - w - 3.0, y + 1.0, w, 18.0];
            (field, [field[0] - 18.0 - 2.0, y + 1.0, 18.0, 18.0])
        }
        // Boolean / Enum / Key: a 160 button less the two buttons.
        _ => {
            let w = 160.0 - 20.0 - 2.0 - 20.0 - 2.0;
            ([reset[0] - w - 2.0, y, w, 20.0], [0.0; 4])
        }
    };
    RowRects { undo, reset, value, swatch }
}
fn inside(p: [f32; 2], r: [f32; 4]) -> bool {
    p[0] >= r[0] && p[0] < r[0] + r[2] && p[1] >= r[1] && p[1] < r[1] + r[3]
}
fn footer_buttons(w: f32, h: f32) -> ([f32; 4], [f32; 4]) {
    let x0 = ((w - 408.0) / 2.0).floor();
    let y = h - FOOTER + 6.0;
    ([x0, y, 200.0, 20.0], [x0 + 208.0, y, 200.0, 20.0])
}
fn confirm_buttons(w: f32, h: f32) -> ([f32; 4], [f32; 4]) {
    let x0 = (w / 2.0 - 155.0).floor();
    let y = (h / 2.0 + 18.0).floor();
    ([x0, y, 150.0, 20.0], [x0 + 160.0, y, 150.0, 20.0])
}

fn value_label(s: &State, e: &Entry) -> Text {
    let v = s.get(&e.key);
    match e.kind {
        Kind::Bool => Text::legacy(if v == "true" { "§aYes" } else { "§cNo" }),
        Kind::Enum(names) => Text::literal(names.iter().find(|(n, _)| *n == v).map_or(v.as_str(), |(_, l)| *l)),
        Kind::Key => {
            let label = horizon_client::keybinds::display_name(&v).plain();
            if s.selected_key.as_deref() == Some(e.key.as_str()) {
                // "> " + key (white, underlined) + " <", yellow.
                Text::legacy(&format!("§e> §f§n{label}§r§e <"))
            } else {
                Text::literal(label)
            }
        }
        _ => Text::literal(v),
    }
}
/// ValueConfigEntry.renderLabel: grey, italic white when changed, red on an error.
fn label(s: &State, e: &Entry) -> Text {
    let style = match (s.error(e).is_some(), s.changed(e)) {
        (true, true) => "§c§o",
        (true, false) => "§c",
        (false, true) => "§f§o",
        (false, false) => "§7",
    };
    Text::legacy(&format!("{style}{}", e.title))
}

// ---------------------------------------------------------------------------------------------
// Drawing (extractRenderState)
// ---------------------------------------------------------------------------------------------

pub fn draw(game: &super::Game<'_>, batch: &mut HudBatch, mouse: [f32; 2]) {
    if !mine(game) {
        return;
    }
    let atlas = &game.gui.atlas;
    let (w, h) = (batch.width, batch.height);
    STATE.with(|cell| {
        let mut st = cell.borrow_mut();
        let Some(s) = st.as_mut() else { return };
        s.size = [w, h];
        ui::menu_background(batch, atlas, 0.0, 0.0, w, h, true);
        if let Some(c) = s.confirm {
            draw_confirm(batch, atlas, c, w, h, mouse);
            return;
        }
        let r = rows(w, h);
        ui::separators(batch, atlas, r.top, r.bottom, true);
        // TabNavigationBar
        let n = s.tabs.len();
        let (x0, tw) = tab_layout(w, n);
        for t in 0..n {
            let x = x0 + tw * t as f32;
            let selected = s.tab == t;
            let hover = ui::hovered((mouse[0], mouse[1]), x, 0.0, tw, TAB_H);
            let sprite = match (selected, hover) {
                (true, true) => "gui/sprites/widget/tab_selected_highlighted",
                (true, false) => "gui/sprites/widget/tab_selected",
                (false, true) => "gui/sprites/widget/tab_highlighted",
                (false, false) => "gui/sprites/widget/tab",
            };
            ui::nine_slice(batch, atlas, sprite, x, 0.0, tw, TAB_H, u32::MAX);
            // ConfigCategoryTab.refresh: italic when changed, red on an error.
            let style = match (s.tab_error(t), s.tab_changed(t)) {
                (true, true) => "§c§o",
                (true, false) => "§c",
                (false, true) => "§o",
                (false, false) => "",
            };
            let title = Text::legacy(&format!("{style}{}", s.tabs[t].0));
            ui::menu_tab_label(batch, atlas, &title, (x, 0.0, tw, TAB_H), selected, true, horizon_client::hud_overlays::now_ms());
            if selected {
                batch.fill(atlas, x + 2.0, TAB_H - 3.0, tw - 4.0, 1.0, u32::MAX);
            }
        }
        // ConfigEntryList
        let max = max_scroll(s, &r);
        let scroll = s.scroll[s.tab].clamp(0.0, max);
        s.scroll[s.tab] = scroll;
        let first = batch.vertices.len();
        let mut tooltip: Option<Vec<Text>> = None;
        let entries = s.tabs[s.tab].1.clone();
        for (i, e) in entries.iter().enumerate() {
            let y = r.top + 4.0 + i as f32 * ROW_H - scroll + 2.0;
            if y + 20.0 < r.top || y > r.bottom {
                continue;
            }
            let row_hover = mouse[1] >= y - 2.0 && mouse[1] < y - 2.0 + ROW_H && mouse[0] >= r.left && mouse[0] < r.right
                && mouse[1] >= r.top && mouse[1] < r.bottom;
            match e.kind {
                Kind::Title => {
                    ui::centered_text(batch, atlas, &Text::literal(e.title.clone()), r.left, y + 5.0, r.right - r.left, 0xFFFF_FFFF);
                }
                Kind::Prefix => {
                    ui::text_scaled(batch, atlas, &Text::legacy(&format!("§7{}", e.key)), r.left, y + 5.0, 1.0, 0xFFFF_FFFF);
                }
                _ => {
                    ui::text_scaled(batch, atlas, &label(s, e), r.left, y + 5.0, 1.0, 0xFFFF_FFFF);
                    let rr = row_rects(e.kind, r.right, y);
                    match e.kind {
                        Kind::Int | Kind::Color => {
                            let v = s.get(&e.key);
                            let field = s.fields.entry(e.key.clone()).or_insert_with(|| {
                                let mut f = ui::EditBox::new(rr.value[0], rr.value[1], rr.value[2], rr.value[3]);
                                f.set_value(&v);
                                f
                            });
                            field.x = rr.value[0];
                            field.y = rr.value[1];
                            field.width = rr.value[2];
                            field.focused = s.focused.as_deref() == Some(e.key.as_str());
                            let red = (e.kind == Kind::Int && !v.trim().parse::<i32>().is_ok_and(|x| x > 0))
                                || (e.kind == Kind::Color && core::parse_color(&v).is_none());
                            let fmt = red.then(|| Text::legacy(&format!("§c{}", field.value)));
                            field.draw(batch, atlas, fmt.as_ref());
                            if e.kind == Kind::Color {
                                // ColorWidget: the text field sprite and the colour inside it.
                                let sw = rr.swatch;
                                let sprite = if field.focused { "gui/sprites/widget/text_field_highlighted" } else { "gui/sprites/widget/text_field" };
                                ui::nine_slice(batch, atlas, sprite, sw[0], sw[1], sw[2], sw[3], u32::MAX);
                                let c = core::parse_color(&v).unwrap_or(0xFFFFFF);
                                let [a, rr_, g, b] = (0xFF00_0000 | c).to_be_bytes();
                                batch.fill(atlas, sw[0] + 1.0, sw[1] + 1.0, sw[2] - 2.0, sw[3] - 2.0, horizon_client::hud::rgba(rr_, g, b, a));
                            }
                        }
                        _ => {
                            let hv = inside(mouse, rr.value);
                            ui::button(batch, atlas, rr.value[0], rr.value[1], rr.value[2], rr.value[3], &value_label(s, e), true, hv);
                        }
                    }
                    let reset_on = !s.is_default(e);
                    let undo_on = s.changed(e);
                    ui::button(batch, atlas, rr.reset[0], rr.reset[1], 20.0, 20.0, &Text::literal("R"), reset_on, inside(mouse, rr.reset));
                    ui::button(batch, atlas, rr.undo[0], rr.undo[1], 20.0, 20.0, &Text::literal("U"), undo_on, inside(mouse, rr.undo));
                    if row_hover {
                        tooltip = Some(if inside(mouse, rr.reset) {
                            vec![Text::literal("Reset to default")]
                        } else if inside(mouse, rr.undo) {
                            vec![Text::literal("Undo change")]
                        } else {
                            let mut lines: Vec<Text> = e.tooltip.split('\n').filter(|l| !e.tooltip.is_empty() || !l.is_empty()).map(|l| Text::literal(l.to_string())).collect();
                            if let Some(err) = s.error(e) {
                                lines.push(Text::legacy(&format!("§c{err}")));
                            }
                            lines
                        });
                    }
                }
            }
        }
        batch.clip_since(first, 0.0, r.top, w, r.bottom);
        if max > 0.0 {
            ui::scrollbar(batch, atlas, r.right + 10.0, r.top, r.bottom, content_height(entries.len()), scroll);
        }
        // Footer: the quit and save buttons.
        let (quit, save) = footer_buttons(w, h);
        let quit_label = if s.any_changed() { "Discard Changes" } else { "Cancel" };
        let save_label = if s.any_error() { "Error!" } else { "Save & Quit" };
        ui::button(batch, atlas, quit[0], quit[1], quit[2], quit[3], &Text::literal(quit_label), true, inside(mouse, quit));
        let save_on = s.any_changed() && !s.any_error();
        ui::button(batch, atlas, save[0], save[1], save[2], save[3], &Text::literal(save_label), save_on, inside(mouse, save));
        if let Some(lines) = tooltip.filter(|l| !l.is_empty()) {
            ui::tooltip(batch, atlas, &lines, (mouse[0], mouse[1]));
        }
    });
}

/// Vanilla's ConfirmScreen: title, message, two buttons.
fn draw_confirm(batch: &mut HudBatch, atlas: &horizon_client::hud::HudAtlas, c: Confirm, w: f32, h: f32, mouse: [f32; 2]) {
    let (title, message, yes, no) = match c {
        Confirm::Quit => ("Changes Not Saved", "Are you sure you want to quit editing the config? Changes will be lost!", "Quit & Discard Changes", "Cancel"),
        Confirm::Restart => ("Restart Required", "One of your modifications requires a restart to take effect.", "Exit Minecraft", "Ignore and Continue"),
    };
    ui::centered_text(batch, atlas, &Text::literal(title), 0.0, (h / 2.0 - 40.0).floor(), w, 0xFFFF_FFFF);
    ui::centered_text(batch, atlas, &Text::literal(message), 0.0, (h / 2.0 - 20.0).floor(), w, 0xFFFF_FFFF);
    let (a, b) = confirm_buttons(w, h);
    ui::button(batch, atlas, a[0], a[1], a[2], a[3], &Text::literal(yes), true, inside(mouse, a));
    ui::button(batch, atlas, b[0], b[1], b[2], b[3], &Text::literal(no), true, inside(mouse, b));
}

// ---------------------------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------------------------

fn cycle(s: &mut State, e: &Entry, back: bool) {
    let v = s.get(&e.key);
    match e.kind {
        Kind::Bool => s.set(&e.key, if v == "true" { "false".into() } else { "true".into() }),
        Kind::Enum(names) => {
            let i = names.iter().position(|(n, _)| *n == v).unwrap_or(0);
            let j = if back { (i + names.len() - 1) % names.len() } else { (i + 1) % names.len() };
            s.set(&e.key, names[j].0.to_string());
        }
        _ => {}
    }
}

/// A press over the screen: buttons, tabs, fields. Always swallowed.
pub fn press(game: &super::Game<'_>, mouse: [f32; 2], button: i32, modifiers: u32) {
    let shift = modifiers & 1 != 0;
    STATE.with(|cell| {
        let mut st = cell.borrow_mut();
        let Some(s) = st.as_mut() else {
            return;
        };
        let [w, h] = s.size;
        if let Some(c) = s.confirm {
            let (a, b) = confirm_buttons(w, h);
            if inside(mouse, a) {
                match c {
                    // Quit & Discard Changes
                    Confirm::Quit => close(),
                    // Exit Minecraft: Horizon keeps running; the screen closes.
                    Confirm::Restart => close(),
                }
            } else if inside(mouse, b) {
                match c {
                    Confirm::Quit => s.confirm = None,
                    Confirm::Restart => close(),
                }
            }
            return;
        }
        // A pending key capture takes a mouse button (InputConstants mouse keys).
        if let Some(key) = s.selected_key.take() {
            let name = match button {
                0 => "key.mouse.left",
                1 => "key.mouse.right",
                2 => "key.mouse.middle",
                3 => "key.mouse.4",
                4 => "key.mouse.5",
                _ => "key.keyboard.unknown",
            };
            s.set(&key, name.into());
            return;
        }
        let r = rows(w, h);
        let (x0, tw) = tab_layout(w, s.tabs.len());
        if mouse[1] < TAB_H {
            for t in 0..s.tabs.len() {
                if ui::hovered((mouse[0], mouse[1]), x0 + tw * t as f32, 0.0, tw, TAB_H) {
                    s.tab = t;
                    s.focused = None;
                }
            }
            return;
        }
        let (quit, save) = footer_buttons(w, h);
        if inside(mouse, quit) {
            on_close(s);
            return;
        }
        if inside(mouse, save) {
            if s.any_changed() && !s.any_error() {
                save_and_quit(game, s);
            }
            return;
        }
        if mouse[1] < r.top || mouse[1] >= r.bottom {
            return;
        }
        let scroll = s.scroll[s.tab];
        s.focused = None;
        let entries = s.tabs[s.tab].1.clone();
        for (i, e) in entries.iter().enumerate() {
            if matches!(e.kind, Kind::Title | Kind::Prefix) {
                continue;
            }
            let y = r.top + 4.0 + i as f32 * ROW_H - scroll + 2.0;
            let rr = row_rects(e.kind, r.right, y);
            if inside(mouse, rr.reset) && !s.is_default(e) {
                // resetToDefault
                let d = s.defaults.get(&e.key).cloned().unwrap_or_default();
                s.set(&e.key, d);
                s.selected_key = None;
            } else if inside(mouse, rr.undo) && s.changed(e) {
                // resetToActive
                let a = s.active.get(&e.key).cloned().unwrap_or_default();
                s.set(&e.key, a);
            } else if inside(mouse, rr.value) {
                match e.kind {
                    Kind::Bool | Kind::Enum(_) => cycle(s, e, shift || button == 1),
                    Kind::Key => s.selected_key = Some(e.key.clone()),
                    Kind::Int | Kind::Color => {
                        s.focused = Some(e.key.clone());
                        if let Some(f) = s.fields.get_mut(&e.key) {
                            f.focused = true;
                            f.click(&game.gui.atlas, mouse[0], shift, false);
                        }
                    }
                    _ => {}
                }
            }
        }
    });
}

pub fn scrolled(mouse: [f32; 2], dy: f64) {
    STATE.with(|cell| {
        let mut st = cell.borrow_mut();
        let Some(s) = st.as_mut() else { return };
        if s.confirm.is_some() {
            return;
        }
        let [w, h] = s.size;
        let r = rows(w, h);
        if mouse[1] >= r.top && mouse[1] < r.bottom {
            let max = max_scroll(s, &r);
            // AbstractScrollArea: 10 px per line of the wheel (scrollRate is the row height / 2).
            let t = s.tab;
            s.scroll[t] = (s.scroll[t] - dy as f32 * ROW_H / 2.0).clamp(0.0, max);
        }
    });
}

/// A key over the screen. True when it was used (Escape included: its close is ours).
pub fn key(name: &str, modifiers: u32) -> bool {
    let ctrl = modifiers & 2 != 0;
    let shift = modifiers & 1 != 0;
    STATE.with(|cell| {
        let mut st = cell.borrow_mut();
        let Some(s) = st.as_mut() else { return false };
        // ConfigCategoryTab.keyPressed: a selected key entry takes the key, Escape unbinds it.
        if let Some(entry) = s.selected_key.take() {
            let v = if name == "key.keyboard.escape" { "key.keyboard.unknown".to_string() } else { name.to_string() };
            s.set(&entry, v);
            return true;
        }
        if name == "key.keyboard.escape" {
            if s.confirm.is_some() {
                s.confirm = None;
            } else {
                on_close(s);
            }
            return true;
        }
        if let Some(focus) = s.focused.clone()
            && let Some(code) = horizon_client::keybinds::key_code(name)
            && let Some(f) = s.fields.get_mut(&focus)
        {
            f.key(code, ctrl, shift);
            let v = f.value.clone();
            s.editing.insert(focus, v);
            return true;
        }
        if name == "key.keyboard.tab" {
            s.tab = (s.tab + 1) % s.tabs.len();
            return true;
        }
        false
    })
}

pub fn typed(c: char) -> bool {
    STATE.with(|cell| {
        let mut st = cell.borrow_mut();
        let Some(s) = st.as_mut() else { return false };
        let Some(focus) = s.focused.clone() else { return false };
        let kind = s.tabs.iter().flat_map(|(_, es)| es).find(|e| e.key == focus).map(|e| e.kind);
        // IntegerValueConfigEntry takes `-?\d*`; the colour field any hex text.
        let ok = match kind {
            Some(Kind::Int) => c.is_ascii_digit() || c == '-',
            Some(Kind::Color) => c.is_ascii_hexdigit() || c == '#' || c == 'x',
            _ => false,
        };
        if ok && let Some(f) = s.fields.get_mut(&focus) {
            f.insert_text(&c.to_string());
            let v = f.value.clone();
            s.editing.insert(focus, v);
        }
        true
    })
}

pub fn is_open(game: &super::Game<'_>) -> bool {
    mine(game) && STATE.with(|s| s.borrow().is_some())
}
