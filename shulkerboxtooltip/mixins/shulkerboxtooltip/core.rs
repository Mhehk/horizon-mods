// ShulkerBoxTooltip — Horizon port (mhv2) by mihehk.
// A line-by-line port of ShulkerBoxTooltip 5.4.3+26.3 by MisterPeModder
// (https://github.com/MisterPeModder/ShulkerBoxTooltip), under its licence:
//
//   MIT License
//   Copyright (c) 2019-2026 Yanis Guaye (MisterPeModder)
//   Permission is hereby granted, free of charge, to any person obtaining a copy of this software
//   and associated documentation files (the "Software"), to deal in the Software without
//   restriction, including without limitation the rights to use, copy, modify, merge, publish,
//   distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
//   Software is furnished to do so, subject to the following conditions:
//   The above copyright notice and this permission notice shall be included in all copies or
//   substantial portions of the Software.
//   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
//   BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
//   NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
//   DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//   OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
//
// Each function names the Java method it ports. Setting keys are the Java field paths
// (`preview.enable`, `controls.previewKey`, ...); colours are `colors.<category>.<id>`.
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::text::Text;
use std::sync::{Arc, Mutex};

pub const ID: &str = "shulkerboxtooltip";
pub const TITLE: &str = "ShulkerBoxTooltip — mhv2";

// ---------------------------------------------------------------------------------------------
// Configuration.java / ClientConfiguration.java
// ---------------------------------------------------------------------------------------------

/// A config enum: its Java constant names, in declaration order.
pub trait ConfigEnum: Copy + PartialEq + 'static {
    const NAMES: &'static [&'static str];
    fn from_index(i: usize) -> Self;
    fn index(self) -> usize;
    fn name(self) -> &'static str {
        Self::NAMES[self.index()]
    }
    fn parse(s: &str) -> Option<Self> {
        Self::NAMES.iter().position(|n| *n == s).map(Self::from_index)
    }
}
macro_rules! config_enum {
    ($name:ident { $($v:ident = $s:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum $name { $($v),+ }
        impl ConfigEnum for $name {
            const NAMES: &'static [&'static str] = &[$($s),+];
            fn from_index(i: usize) -> Self { [$(Self::$v),+][i] }
            fn index(self) -> usize { [$(Self::$v),+].iter().position(|v| *v == self).unwrap_or(0) }
        }
    };
}
config_enum!(MergingStrategy { Ignore = "IGNORE", FirstItem = "FIRST_ITEM", Separate = "SEPARATE" });
config_enum!(CompactOrder { StackSize = "STACK_SIZE", PreviewOrder = "PREVIEW_ORDER" });
config_enum!(Theme { ShulkerBoxTooltip = "SHULKERBOXTOOLTIP", Vanilla = "VANILLA" });
config_enum!(BundleTheme { Vanilla = "VANILLA", VanillaPlus = "VANILLA_PLUS", ShulkerBoxTooltip = "SHULKERBOXTOOLTIP" });
config_enum!(Position { Inside = "INSIDE", Outside = "OUTSIDE", OutsideTop = "OUTSIDE_TOP", OutsideBottom = "OUTSIDE_BOTTOM" });
config_enum!(TooltipType { Vanilla = "VANILLA", Mod = "MOD", None = "NONE" });
config_enum!(LootTableInfo { Hide = "HIDE", Simple = "SIMPLE", Advanced = "ADVANCED" });
config_enum!(EnderChestSync { None = "NONE", Active = "ACTIVE", Passive = "PASSIVE" });

/// One colour of the registry (`ShulkerBoxTooltip.registerColors`).
pub struct ColorEntry {
    /// The config category (`default`, `shulker_boxes`, `bundles`).
    pub category: &'static str,
    pub id: &'static str,
    /// `ColorKey.ofRgb(x)`, or a dye's texture diffuse colour for `ColorKey.ofDye`.
    rgb: u32,
    dye: bool,
}
impl ColorEntry {
    /// `ColorKey.ofRgb` keeps the colour; `ColorKey.ofDye` raises each channel to at least
    /// 0.15, and `rgb()` is `componentsToRgb` (`(int) (255F * c)`).
    pub fn default_rgb(&self) -> u32 {
        let mut out = 0;
        for shift in [16u32, 8, 0] {
            let c = ((self.rgb >> shift) & 0xFF) as f32 / 255.0;
            let c = if self.dye { c.max(0.15) } else { c };
            out |= ((255.0f32 * c) as u32) << shift;
        }
        out
    }
}
const fn rgb(category: &'static str, id: &'static str, rgb: u32) -> ColorEntry {
    ColorEntry { category, id, rgb, dye: false }
}
const fn dye(category: &'static str, id: &'static str, rgb: u32) -> ColorEntry {
    ColorEntry { category, id, rgb, dye: true }
}
/// `DyeColor.getTextureDiffuseColor`, by dye name.
const DYES: [(&str, u32); 16] = [
    ("white", 0xF9FFFE),
    ("orange", 0xF9801D),
    ("magenta", 0xC74EBD),
    ("light_blue", 0x3AB3DA),
    ("yellow", 0xFED83D),
    ("lime", 0x80C71F),
    ("pink", 0xF38BAA),
    ("gray", 0x474F52),
    ("light_gray", 0x9D9D97),
    ("cyan", 0x169C9C),
    ("purple", 0x8932B8),
    ("blue", 0x3C44AA),
    ("brown", 0x835432),
    ("green", 0x5E7C16),
    ("red", 0xB02E26),
    ("black", 0x1D1D21),
];
pub static COLORS: [ColorEntry; 37] = [
    rgb("default", "default", 0xFFFFFF),
    rgb("default", "ender_chest", 0x0B4B41),
    rgb("default", "generic_container", 0xFFFFFF),
    rgb("shulker_boxes", "shulker_box", 0x976797),
    dye("shulker_boxes", "white_shulker_box", DYES[0].1),
    dye("shulker_boxes", "orange_shulker_box", DYES[1].1),
    dye("shulker_boxes", "magenta_shulker_box", DYES[2].1),
    dye("shulker_boxes", "light_blue_shulker_box", DYES[3].1),
    dye("shulker_boxes", "yellow_shulker_box", DYES[4].1),
    dye("shulker_boxes", "lime_shulker_box", DYES[5].1),
    dye("shulker_boxes", "pink_shulker_box", DYES[6].1),
    dye("shulker_boxes", "gray_shulker_box", DYES[7].1),
    dye("shulker_boxes", "light_gray_shulker_box", DYES[8].1),
    dye("shulker_boxes", "cyan_shulker_box", DYES[9].1),
    dye("shulker_boxes", "purple_shulker_box", DYES[10].1),
    dye("shulker_boxes", "blue_shulker_box", DYES[11].1),
    dye("shulker_boxes", "brown_shulker_box", DYES[12].1),
    dye("shulker_boxes", "green_shulker_box", DYES[13].1),
    dye("shulker_boxes", "red_shulker_box", DYES[14].1),
    dye("shulker_boxes", "black_shulker_box", DYES[15].1),
    rgb("bundles", "bundle", 0x844B2B),
    dye("bundles", "white_bundle", DYES[0].1),
    dye("bundles", "orange_bundle", DYES[1].1),
    dye("bundles", "magenta_bundle", DYES[2].1),
    dye("bundles", "light_blue_bundle", DYES[3].1),
    dye("bundles", "yellow_bundle", DYES[4].1),
    dye("bundles", "lime_bundle", DYES[5].1),
    dye("bundles", "pink_bundle", DYES[6].1),
    dye("bundles", "gray_bundle", DYES[7].1),
    dye("bundles", "light_gray_bundle", DYES[8].1),
    dye("bundles", "cyan_bundle", DYES[9].1),
    dye("bundles", "purple_bundle", DYES[10].1),
    dye("bundles", "blue_bundle", DYES[11].1),
    dye("bundles", "brown_bundle", DYES[12].1),
    dye("bundles", "green_bundle", DYES[13].1),
    dye("bundles", "red_bundle", DYES[14].1),
    dye("bundles", "black_bundle", DYES[15].1),
];
pub fn color_index(id: &str) -> usize {
    COLORS.iter().position(|c| c.id == id).unwrap_or(0)
}
pub fn color_setting_key(c: &ColorEntry) -> String {
    format!("colors.{}.{}", c.category, c.id)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    // PreviewCategory
    pub preview_enable: bool,
    pub swap_modes: bool,
    pub always_on: bool,
    pub compact_preview_nbt_behavior: MergingStrategy,
    pub compact_preview_order: CompactOrder,
    pub default_max_row_size: i32,
    pub server_integration: bool,
    pub theme: Theme,
    pub theme_bundle: BundleTheme,
    pub position: Position,
    pub short_item_counts: bool,
    pub generic_container_preview: bool,
    // TooltipCategory
    pub show_key_hints: bool,
    pub tooltip_type: TooltipType,
    pub loot_table_info_type: LootTableInfo,
    pub hide_shulker_box_lore: bool,
    // ColorsCategory
    pub colored_preview: bool,
    pub colors: Vec<u32>,
    // ControlsCategory
    pub preview_key: String,
    pub full_preview_key: String,
    pub lock_tooltip_key: String,
    // ServerCategory
    pub client_integration: bool,
    pub ender_chest_sync_type: EnderChestSync,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            preview_enable: true,
            swap_modes: false,
            always_on: false,
            compact_preview_nbt_behavior: MergingStrategy::Separate,
            compact_preview_order: CompactOrder::StackSize,
            default_max_row_size: 9,
            server_integration: true,
            theme: Theme::ShulkerBoxTooltip,
            theme_bundle: BundleTheme::Vanilla,
            position: Position::Inside,
            short_item_counts: true,
            generic_container_preview: true,
            show_key_hints: true,
            tooltip_type: TooltipType::Mod,
            loot_table_info_type: LootTableInfo::Hide,
            hide_shulker_box_lore: false,
            colored_preview: true,
            colors: COLORS.iter().map(ColorEntry::default_rgb).collect(),
            preview_key: "key.keyboard.left.shift".into(),
            full_preview_key: "key.keyboard.left.alt".into(),
            lock_tooltip_key: "key.keyboard.left.control".into(),
            client_integration: true,
            ender_chest_sync_type: EnderChestSync::Active,
        }
    }
}

fn parse_bool(s: Option<String>, d: bool) -> bool {
    match s.as_deref().map(str::trim) {
        Some("true") => true,
        Some("false") => false,
        _ => d,
    }
}
fn parse_enum<E: ConfigEnum>(s: Option<String>, d: E) -> E {
    s.and_then(|s| E::parse(s.trim())).unwrap_or(d)
}
/// `JsonHexadecimalInt`: `#RRGGBB` or `0xRRGGBB`.
pub fn parse_color(s: &str) -> Option<u32> {
    let t = s.trim();
    let hex = t.strip_prefix('#').or_else(|| t.strip_prefix("0x")).unwrap_or(t);
    if hex.is_empty() || hex.len() > 6 {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}
pub fn format_color(c: u32) -> String {
    format!("#{:06X}", c & 0xFF_FFFF)
}

impl Config {
    /// `ConfigurationHandler` reading: each field from its setting, the default when absent
    /// or invalid (`GreaterThanZero` for the row size).
    pub fn load(get: &dyn Fn(&str) -> Option<String>) -> Config {
        let d = Config::default();
        let row = get("preview.defaultMaxRowSize")
            .and_then(|s| s.trim().parse::<i32>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(d.default_max_row_size);
        Config {
            preview_enable: parse_bool(get("preview.enable"), d.preview_enable),
            swap_modes: parse_bool(get("preview.swapModes"), d.swap_modes),
            always_on: parse_bool(get("preview.alwaysOn"), d.always_on),
            compact_preview_nbt_behavior: parse_enum(get("preview.compactPreviewNbtBehavior"), d.compact_preview_nbt_behavior),
            compact_preview_order: parse_enum(get("preview.compactPreviewOrder"), d.compact_preview_order),
            default_max_row_size: row,
            server_integration: parse_bool(get("preview.serverIntegration"), d.server_integration),
            theme: parse_enum(get("preview.theme"), d.theme),
            theme_bundle: parse_enum(get("preview.themeBundle"), d.theme_bundle),
            position: parse_enum(get("preview.position"), d.position),
            short_item_counts: parse_bool(get("preview.shortItemCounts"), d.short_item_counts),
            generic_container_preview: parse_bool(get("preview.genericContainerPreview"), d.generic_container_preview),
            show_key_hints: parse_bool(get("tooltip.showKeyHints"), d.show_key_hints),
            tooltip_type: parse_enum(get("tooltip.type"), d.tooltip_type),
            loot_table_info_type: parse_enum(get("tooltip.lootTableInfoType"), d.loot_table_info_type),
            hide_shulker_box_lore: parse_bool(get("tooltip.hideShulkerBoxLore"), d.hide_shulker_box_lore),
            colored_preview: parse_bool(get("colors.coloredPreview"), d.colored_preview),
            colors: COLORS
                .iter()
                .zip(&d.colors)
                .map(|(c, def)| get(&color_setting_key(c)).and_then(|s| parse_color(&s)).unwrap_or(*def))
                .collect(),
            preview_key: get("controls.previewKey").filter(|s| !s.trim().is_empty()).unwrap_or(d.preview_key),
            full_preview_key: get("controls.fullPreviewKey").filter(|s| !s.trim().is_empty()).unwrap_or(d.full_preview_key),
            lock_tooltip_key: get("controls.lockTooltipKey").filter(|s| !s.trim().is_empty()).unwrap_or(d.lock_tooltip_key),
            client_integration: parse_bool(get("server.clientIntegration"), d.client_integration),
            ender_chest_sync_type: parse_enum(get("server.enderChestSyncType"), d.ender_chest_sync_type),
        }
    }
    /// Every setting as written to disk, key and value.
    pub fn entries(&self) -> Vec<(String, String)> {
        let b = |v: bool| v.to_string();
        let mut out = vec![
            ("preview.enable".into(), b(self.preview_enable)),
            ("preview.swapModes".into(), b(self.swap_modes)),
            ("preview.alwaysOn".into(), b(self.always_on)),
            ("preview.compactPreviewNbtBehavior".into(), self.compact_preview_nbt_behavior.name().into()),
            ("preview.compactPreviewOrder".into(), self.compact_preview_order.name().into()),
            ("preview.defaultMaxRowSize".into(), self.default_max_row_size.to_string()),
            ("preview.serverIntegration".into(), b(self.server_integration)),
            ("preview.theme".into(), self.theme.name().into()),
            ("preview.themeBundle".into(), self.theme_bundle.name().into()),
            ("preview.position".into(), self.position.name().into()),
            ("preview.shortItemCounts".into(), b(self.short_item_counts)),
            ("preview.genericContainerPreview".into(), b(self.generic_container_preview)),
            ("tooltip.showKeyHints".into(), b(self.show_key_hints)),
            ("tooltip.type".into(), self.tooltip_type.name().into()),
            ("tooltip.lootTableInfoType".into(), self.loot_table_info_type.name().into()),
            ("tooltip.hideShulkerBoxLore".into(), b(self.hide_shulker_box_lore)),
            ("colors.coloredPreview".into(), b(self.colored_preview)),
            ("controls.previewKey".into(), self.preview_key.clone()),
            ("controls.fullPreviewKey".into(), self.full_preview_key.clone()),
            ("controls.lockTooltipKey".into(), self.lock_tooltip_key.clone()),
            ("server.clientIntegration".into(), b(self.client_integration)),
            ("server.enderChestSyncType".into(), self.ender_chest_sync_type.name().into()),
        ];
        for (c, v) in COLORS.iter().zip(&self.colors) {
            out.push((color_setting_key(c), format_color(*v)));
        }
        out
    }
}

struct Cache {
    config: Option<Arc<Config>>,
    /// The colours the uploaded atlas was baked with (None: not uploaded).
    atlas: Option<Vec<u32>>,
}
static CACHE: Mutex<Cache> = Mutex::new(Cache { config: None, atlas: None });

/// `ShulkerBoxTooltipClient.getConfig`: read once, again after the config screen saves.
pub fn config(game: &super::Game<'_>) -> Arc<Config> {
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(c) = &cache.config {
        return c.clone();
    }
    let c = Arc::new(Config::load(&|key| game.native_setting(ID, key)));
    cache.config = Some(c.clone());
    c
}
/// The settings are read again on next use (after the config screen saved them).
pub fn reload_config() {
    CACHE.lock().unwrap_or_else(|e| e.into_inner()).config = None;
}
/// `ConfigurationHandler.saveToFile` + reload.
pub fn save_config(game: &super::Game<'_>, c: &Config) {
    for (k, v) in c.entries() {
        game.native_set_setting(ID, &k, &v);
    }
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache.config = Some(Arc::new(c.clone()));
}

// ---------------------------------------------------------------------------------------------
// Keys (ShulkerBoxTooltipClient.updatePreviewKeys / isKeyPressed): the raw key state.
// ---------------------------------------------------------------------------------------------

/// `InputConstants.isKeyDown(key)`: the key held now (a mouse button for `key.mouse.*`).
pub fn key_down(game: &super::Game<'_>, key: &str) -> bool {
    if key.is_empty() || key == "key.keyboard.unknown" {
        return false;
    }
    // Shift, Control and Alt from the live modifier word, as Horizon's own screens read them
    // (`has_shift_down`): a release lost while unfocused can't leave one stuck.
    match key {
        "key.keyboard.left.shift" | "key.keyboard.right.shift" => return game.modifiers.shift_key(),
        "key.keyboard.left.control" | "key.keyboard.right.control" => return game.modifiers.control_key(),
        "key.keyboard.left.alt" | "key.keyboard.right.alt" => return game.modifiers.alt_key(),
        _ => {}
    }
    if let Some(button) = horizon_client::keybinds::mouse_button(key) {
        return game.physical_buttons.contains(&button);
    }
    horizon_client::keybinds::key_code(key).is_some_and(|code| game.physical_keys.contains(&code))
}
/// `Key.get().getDisplayName()`.
fn key_label(key: &str) -> String {
    horizon_client::keybinds::display_name(key).plain()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PreviewType {
    NoPreview,
    Compact,
    Full,
}

pub struct Keys {
    pub preview: bool,
    pub full: bool,
    pub lock: bool,
}
pub fn keys(game: &super::Game<'_>, c: &Config) -> Keys {
    Keys {
        preview: key_down(game, &c.preview_key),
        full: key_down(game, &c.full_preview_key),
        lock: key_down(game, &c.lock_tooltip_key),
    }
}

/// `ShulkerBoxTooltipClient.getCurrentPreviewType`.
pub fn current_preview_type(c: &Config, k: &Keys, has_full_preview_mode: bool) -> PreviewType {
    let requested = c.always_on || k.preview;
    if requested && !has_full_preview_mode {
        return PreviewType::Compact;
    }
    if c.swap_modes {
        if requested {
            return if k.full { PreviewType::Compact } else { PreviewType::Full };
        }
    } else if requested {
        return if k.full { PreviewType::Full } else { PreviewType::Compact };
    }
    PreviewType::NoPreview
}

// ---------------------------------------------------------------------------------------------
// Providers (ShulkerBoxTooltip.registerProviders and the provider classes)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// ShulkerBoxPreviewProvider.
    ShulkerBox,
    /// InventoryAwarePreviewProvider (chests, furnaces, hoppers...).
    Inventory,
    /// GenericContainerPreviewProvider.
    Generic,
}

#[derive(Clone, Copy, Debug)]
pub struct Provider {
    pub kind: Kind,
    /// `getInventoryMaxSize`.
    pub max_size: usize,
    /// `getMaxRowSize` / `getCompactMaxRowSize`.
    pub row: i32,
    pub compact_row: i32,
    /// `canUseLootTables`.
    pub loot: bool,
    /// `getWindowColorKey`, an index into `COLORS`.
    pub color: usize,
}

const SHELVES: [&str; 11] = [
    "acacia", "bamboo", "birch", "cherry", "crimson", "dark_oak", "jungle", "mangrove", "oak", "pale_oak", "spruce",
];
const COPPER: [&str; 8] = ["", "exposed_", "weathered_", "oxidized_", "waxed_", "waxed_exposed_", "waxed_weathered_", "waxed_oxidized_"];

/// `ShulkerBoxTooltipApi.getPreviewProviderForStackWithOverrides`: the provider registered for
/// the stack's item, else the generic container one for any stack with `minecraft:container`.
pub fn provider_for(c: &Config, stack: &ItemStack) -> Option<Provider> {
    let name = stack.item.trim_start_matches("minecraft:");
    // FixedPreviewProviderRegistry(ShulkerBoxPreviewProvider::new): 9 a row, a 27 slot box.
    if name == "shulker_box" {
        return Some(Provider { kind: Kind::ShulkerBox, max_size: 27, row: 9, compact_row: 9, loot: true, color: color_index("shulker_box") });
    }
    if let Some(dye) = name.strip_suffix("_shulker_box")
        && DYES.iter().any(|(d, _)| *d == dye)
    {
        return Some(Provider { kind: Kind::ShulkerBox, max_size: 27, row: 9, compact_row: 9, loot: true, color: color_index(name) });
    }
    // FixedPreviewProviderRegistry(InventoryAwarePreviewProvider::new): (row size, container size,
    // RandomizableContainer).
    let inventory = |row: i32, size: usize, loot: bool| {
        Some(Provider { kind: Kind::Inventory, max_size: size, row, compact_row: row, loot, color: 0 })
    };
    let copper = COPPER.iter().any(|p| name == format!("{p}copper_chest"));
    let shelf = SHELVES.iter().any(|w| name == format!("{w}_shelf"));
    let found = match name {
        "chest" | "trapped_chest" | "barrel" => inventory(9, 27, true),
        _ if copper => inventory(9, 27, true),
        "furnace" | "blast_furnace" | "smoker" => inventory(3, 3, false),
        "dropper" | "dispenser" => inventory(3, 9, true),
        "hopper" => inventory(5, 5, true),
        "brewing_stand" => inventory(5, 5, false),
        "chiseled_bookshelf" => inventory(3, 6, false),
        "decorated_pot" => inventory(1, 1, true),
        _ if shelf => inventory(3, 3, false),
        _ => None,
    };
    if found.is_some() {
        return found;
    }
    // GenericContainerPreviewProvider: super(27, true, 9, 9), priority 0.
    if c.generic_container_preview && has_container(stack) {
        return Some(Provider { kind: Kind::Generic, max_size: 27, row: 9, compact_row: 9, loot: true, color: color_index("generic_container") });
    }
    None
}

/// `stack.has(DataComponents.CONTAINER)`.
fn has_container(stack: &ItemStack) -> bool {
    stack.data().is_some_and(|d| !d.container.is_empty())
}
/// `stack.get(DataComponents.CONTAINER_LOOT)`'s loot table id.
fn container_loot(stack: &ItemStack) -> Option<String> {
    use horizon_world::nbt::Tag;
    let tag = stack.data()?.other("minecraft:container_loot")?;
    match tag.get("loot_table") {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => Some(String::new()),
    }
}

/// `BlockEntityPreviewProvider.getInventory`: `container.copyInto(NonNullList.withSize(max))`.
pub fn inventory(p: &Provider, stack: &ItemStack) -> Vec<Option<ItemStack>> {
    let mut inv = vec![None; p.max_size];
    if let Some(d) = stack.data() {
        for (slot, s) in &d.container {
            if let Some(cell) = inv.get_mut(usize::from(*slot))
                && s.count > 0
            {
                *cell = Some(s.clone());
            }
        }
    }
    inv
}
/// `BlockEntityPreviewProvider.getItemCount`.
fn item_count(inv: &[Option<ItemStack>]) -> usize {
    inv.iter().filter(|s| s.as_ref().is_some_and(|s| &*s.item != "minecraft:air")).count()
}
/// `BlockEntityPreviewProvider.shouldDisplay`.
pub fn should_display(p: &Provider, stack: &ItemStack) -> bool {
    if p.loot && container_loot(stack).is_some() {
        return false;
    }
    item_count(&inventory(p, stack)) > 0
}
/// `showTooltipHints`: shulker boxes always, inventory providers when they display, the
/// generic one when the stack has a container.
fn show_tooltip_hints(p: &Provider, stack: &ItemStack) -> bool {
    match p.kind {
        Kind::ShulkerBox => true,
        Kind::Inventory => should_display(p, stack),
        Kind::Generic => has_container(stack),
    }
}

const GRAY: &str = "§7";
const GOLD: &str = "§6";
const WHITE: &str = "§f";

/// `ShulkerBoxPreviewProvider.addTooltip` / `BlockEntityPreviewProvider.addTooltip`.
fn add_tooltip(c: &Config, k: &Keys, p: &Provider, stack: &ItemStack) -> Vec<Text> {
    let loot = container_loot(stack);
    if p.kind == Kind::ShulkerBox && p.loot && c.loot_table_info_type == LootTableInfo::Hide && loot.is_some() {
        return vec![Text::legacy(&format!("{GRAY}???????"))];
    }
    if p.loot && let Some(id) = loot {
        return match c.loot_table_info_type {
            LootTableInfo::Hide => Vec::new(),
            LootTableInfo::Simple => vec![Text::legacy(&format!("{GRAY}Has loot table"))],
            LootTableInfo::Advanced => vec![Text::legacy("Loot Table: "), Text::legacy(&format!("{GRAY} {id}"))],
        };
    }
    if current_preview_type(c, k, true) == PreviewType::Full {
        return Vec::new();
    }
    // getItemListTooltip
    let n = item_count(&inventory(p, stack));
    if n > 0 {
        vec![Text::legacy(&format!("{GRAY}Contains {n} stack(s)"))]
    } else {
        vec![Text::legacy(&format!("{GRAY}Empty"))]
    }
}

/// `ShulkerBoxTooltipClient.getPreviewKeyTooltipHint`.
fn preview_key_hint(c: &Config, k: &Keys) -> Option<Text> {
    let requested = c.always_on || k.preview;
    if requested && k.full {
        return None;
    }
    let preview = key_label(&c.preview_key);
    let mut key = String::new();
    if requested {
        key.push_str(&key_label(&c.full_preview_key));
        if !c.always_on {
            key.push('+');
            key.push_str(&preview);
        }
    } else {
        key.push_str(&preview);
    }
    let hint = if current_preview_type(c, k, true) == PreviewType::NoPreview {
        if c.swap_modes { "view full contents" } else { "view contents" }
    } else if c.swap_modes {
        "view contents"
    } else {
        "view full contents"
    };
    Some(Text::legacy(&format!("{GOLD}{key}: {WHITE}{hint}")))
}

/// `ShulkerBoxTooltipClient.modifyStackTooltip`: lines added at the tooltip's end.
pub fn tooltip_lines(game: &super::Game<'_>, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    let c = config(game);
    let Some(p) = provider_for(&c, stack) else {
        return;
    };
    if !show_tooltip_hints(&p, stack) {
        return;
    }
    let k = keys(game, &c);
    if c.tooltip_type == TooltipType::Mod {
        for t in add_tooltip(&c, &k, &p, stack) {
            lines.push((t, u32::MAX));
        }
    }
    // getTooltipHints
    if c.show_key_hints && c.preview_enable && should_display(&p, stack) {
        if let Some(h) = preview_key_hint(&c, &k) {
            lines.push((h, u32::MAX));
        }
        // getLockKeyTooltipHint: only in a container screen's tooltip (lockKeyHintsEnabled).
        let requested = c.always_on || k.preview;
        if requested && !k.lock && LOCK_HINTS.with(|h| h.get()) {
            let key = key_label(&c.lock_tooltip_key);
            lines.push((Text::legacy(&format!("{GOLD}{key}: {WHITE}lock tooltip")), u32::MAX));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Tooltip lock (AbstractContainerScreenMixin): holding the lock key keeps the tooltip on its
// slot and position, and the preview's items show their own tooltips under the mouse.
// ---------------------------------------------------------------------------------------------

#[derive(Clone)]
struct Lock {
    slot: usize,
    x: f32,
    y: f32,
    creative: bool,
    after_name: Vec<(Text, u32)>,
}
thread_local! {
    static LOCK: std::cell::RefCell<Option<Lock>> = const { std::cell::RefCell::new(None) };
    /// The preview slot's stack under the mouse, recorded while drawing (drawInnerTooltip).
    static INNER: std::cell::RefCell<Option<ItemStack>> = const { std::cell::RefCell::new(None) };
    /// Our own nested tooltip draws, which the lock leaves alone.
    static NESTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// `setLockKeyHintsEnabled`: on while a container screen draws its tooltip.
    static LOCK_HINTS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `forceFocusSlot`: while locked, the screen hovers the locked slot.
pub fn locked_slot() -> Option<usize> {
    LOCK.with(|l| l.borrow().as_ref().map(|l| l.slot))
}
fn clear_lock() {
    LOCK.with(|l| *l.borrow_mut() = None);
    INNER.with(|i| *i.borrow_mut() = None);
}
/// `ShulkerBoxTooltipApi.isPreviewAvailable`.
fn preview_available(game: &super::Game<'_>, stack: &ItemStack) -> bool {
    Preview::of(game, stack).is_some()
}

/// `lockTooltipPosition` + `renderLockedTooltip`, at the head of the screens' tooltip draw.
/// True when it drew the tooltip itself (the original draw is skipped).
pub fn tooltip_head(
    game: &mut super::Game<'_>,
    batch: &mut HudBatch,
    stack: &ItemStack,
    creative: bool,
    x: f32,
    y: f32,
    after_name: &[(Text, u32)],
) -> bool {
    if NESTED.with(|n| n.get()) {
        return false;
    }
    LOCK_HINTS.with(|h| h.set(true));
    let c = config(game);
    if !keys(game, &c).lock {
        clear_lock();
        return false;
    }
    if LOCK.with(|l| l.borrow().is_none()) {
        // When locking is requested and no slot is currently locked: the hovered one.
        let Some(slot) = game.slot_at(batch, x, y) else {
            return false;
        };
        if !preview_available(game, stack) {
            return false;
        }
        LOCK.with(|l| {
            *l.borrow_mut() = Some(Lock { slot, x, y, creative, after_name: after_name.to_vec() })
        });
    } else if !preview_available(game, stack) {
        // The locked slot no longer holds a previewable item.
        clear_lock();
        return false;
    }
    let Some(lock) = LOCK.with(|l| l.borrow().clone()) else {
        return false;
    };
    // The locked tooltip, drawn at once at the locked position (renderTooltipImmediate).
    INNER.with(|i| *i.borrow_mut() = None);
    NESTED.with(|n| n.set(true));
    game.draw_stack_tooltip_with(batch, stack, lock.creative, lock.x, lock.y, lock.after_name);
    // BasePreviewRenderer.drawInnerTooltip: the hovered preview slot's own tooltip.
    if let Some(inner) = INNER.with(|i| i.borrow_mut().take()) {
        let cursor = game.cursor;
        game.draw_stack_tooltip_with(batch, &inner, false, cursor.0, cursor.1, Vec::new());
    }
    NESTED.with(|n| n.set(false));
    true
}

/// Each frame: the lock ends with its key, a carried stack, or the screen.
fn frame_lock(game: &super::Game<'_>, c: &Config) {
    LOCK_HINTS.with(|h| h.set(false));
    if !key_down(game, &c.lock_tooltip_key) || game.carried.is_some() || game.screen == super::Screen::Playing {
        clear_lock();
    }
}

// ---------------------------------------------------------------------------------------------
// MergedItemStack.mergeInventory
// ---------------------------------------------------------------------------------------------

pub struct Merged {
    pub stack: ItemStack,
    pub first_slot: usize,
}
/// `ItemKey`: the item, and its components unless they are ignored.
fn same_key(a: &ItemStack, b: &ItemStack, ignore_components: bool) -> bool {
    a.item == b.item && (ignore_components || (a.damage == b.damage && a.extra == b.extra))
}
pub fn merge_inventory(inv: &[Option<ItemStack>], strategy: MergingStrategy, order: CompactOrder) -> Vec<Merged> {
    let ignore = strategy != MergingStrategy::Separate;
    let mut out: Vec<Merged> = Vec::new();
    for (i, s) in inv.iter().enumerate() {
        let Some(s) = s else { continue };
        match out.iter_mut().find(|m| same_key(&m.stack, s, ignore)) {
            // MergedItemStack.add: the merged stack grows.
            Some(m) => m.stack.count += s.count,
            None => {
                let stack = if strategy == MergingStrategy::Ignore {
                    // copyStackWithoutComponents
                    ItemStack { item: s.item.clone(), count: s.count, damage: 0, extra: None }
                } else {
                    s.clone()
                };
                out.push(Merged { stack, first_slot: i });
            }
        }
    }
    match order {
        // Comparator.reverseOrder() of compareTo: count desc, then first slot asc.
        CompactOrder::StackSize => out.sort_by(|a, b| b.stack.count.cmp(&a.stack.count).then(a.first_slot.cmp(&b.first_slot))),
        CompactOrder::PreviewOrder => out.sort_by_key(|m| m.first_slot),
    }
    out
}

/// `ShulkerBoxTooltipUtil.abbreviateInteger`.
pub fn abbreviate_integer(count: i64) -> String {
    let count = count as i32;
    if count == i32::MIN {
        return "-2G".into();
    }
    if count > -1000 && count < 1000 {
        return count.to_string();
    }
    let mut s = String::new();
    let mut count = count;
    if count < 0 {
        s.push('-');
        count = -count;
    }
    let (integral, decimal, unit) = match (count as f64).log10() as i32 {
        3 => (count / 1_000, (count % 1_000) / 100, 'k'),
        4 | 5 => (count / 1_000, 0, 'k'),
        6 => (count / 1_000_000, (count % 1_000_000) / 100_000, 'M'),
        7 | 8 => (count / 1_000_000, 0, 'M'),
        _ => (count / 1_000_000_000, (count % 1_000_000_000) / 100_000_000, 'G'),
    };
    s.push_str(&integral.to_string());
    if decimal > 0 {
        s.push('.');
        s.push_str(&decimal.to_string());
    }
    s.push(unit);
    s
}

// ---------------------------------------------------------------------------------------------
// Rendering (BasePreviewRenderer, ModPreviewRenderer, VanillaPreviewRenderer,
// PreviewClientTooltipComponent)
// ---------------------------------------------------------------------------------------------

/// What `setPreview` + `setPreviewType` give a renderer.
pub struct Preview {
    pub c: Arc<Config>,
    pub p: Provider,
    pub kind: PreviewType,
    pub full: Vec<Option<ItemStack>>,
    pub compact: Vec<Merged>,
    pub max_row: i32,
    pub compact_row: i32,
}
impl Preview {
    /// `getProviderIfPreviewAvailable` + `PreviewClientTooltipComponent`'s constructor.
    pub fn of(game: &super::Game<'_>, stack: &ItemStack) -> Option<Preview> {
        let c = config(game);
        if !c.preview_enable {
            return None;
        }
        let p = provider_for(&c, stack)?;
        if !should_display(&p, stack) {
            return None;
        }
        let k = keys(game, &c);
        let kind = current_preview_type(&c, &k, true);
        if kind == PreviewType::NoPreview {
            return None;
        }
        // BasePreviewRenderer.setPreview
        let mut compact_row = p.compact_row;
        if compact_row <= 0 {
            compact_row = c.default_max_row_size;
        }
        if compact_row <= 0 {
            compact_row = 9;
        }
        let mut row = p.row;
        if row <= 0 {
            row = compact_row;
        }
        let full = inventory(&p, stack);
        let compact = merge_inventory(&full, c.compact_preview_nbt_behavior, c.compact_preview_order);
        Some(Preview { c, p, kind, full, compact, max_row: row, compact_row })
    }
    fn row_size(&self) -> i32 {
        let r = if self.kind == PreviewType::Compact { self.compact_row } else { self.max_row };
        r.max(1)
    }
    /// ModPreviewRenderer.getInvSize
    fn inv_size(&self) -> i32 {
        if self.kind == PreviewType::Compact { (self.compact.len() as i32).max(1) } else { self.p.max_size as i32 }
    }
    /// VanillaPreviewRenderer.getInvSize: lastNonEmptySlot + 1 in full mode.
    fn vanilla_inv_size(&self) -> i32 {
        if self.kind == PreviewType::Compact {
            (self.compact.len() as i32).max(1)
        } else {
            self.full.iter().rposition(Option::is_some).map_or(0, |i| i as i32 + 1)
        }
    }
    fn vanilla_row_size(&self) -> i32 {
        self.row_size().min(self.vanilla_inv_size()).max(1)
    }
    pub fn width(&self) -> i32 {
        match self.c.theme {
            Theme::ShulkerBoxTooltip => 14 + self.row_size().min(self.inv_size()) * 18,
            Theme::Vanilla => self.vanilla_row_size() * 24,
        }
    }
    pub fn height(&self) -> i32 {
        match self.c.theme {
            Theme::ShulkerBoxTooltip => 14 + ceil_div(self.inv_size(), self.row_size()) * 18,
            Theme::Vanilla => ceil_div(self.vanilla_inv_size(), self.vanilla_row_size()) * 24,
        }
    }
    fn color(&self) -> u32 {
        if self.c.colored_preview { self.c.colors[self.p.color] } else { self.c.colors[0] }
    }
}
fn ceil_div(a: i32, b: i32) -> i32 {
    (a as f64 / b as f64).ceil() as i32
}

/// `PreviewClientTooltipComponent.getWidth/getHeight`: the room taken inside the tooltip.
pub fn component_size(game: &super::Game<'_>, stack: &ItemStack, size: &mut Option<[f32; 2]>) {
    if size.is_some() {
        return;
    }
    let Some(pv) = Preview::of(game, stack) else {
        return;
    };
    if pv.compact.is_empty() {
        return;
    }
    if pv.c.position == Position::Inside {
        *size = Some([pv.width() as f32, (pv.height() + 4) as f32]);
    }
}

/// The atlas the windows are drawn from: the nine-slice sprite tinted once per colour key
/// (`blitSprite(..., color)` multiplies the texture), 32 cells a row.
const ATLAS_COLS: u32 = 32;
fn atlas_cell(i: usize) -> [f32; 2] {
    let i = i as u32;
    [((i % ATLAS_COLS) * super::shulkerboxtooltip_texture::WIDTH) as f32, ((i / ATLAS_COLS) * super::shulkerboxtooltip_texture::HEIGHT) as f32]
}
/// Bakes and uploads the atlas when the colours changed (`frame`, which may write).
pub fn frame(game: &mut super::Game<'_>) {
    let c = config(game);
    horizon_client::item_tooltip::shulkerboxtooltip_flags::set(c.tooltip_type != TooltipType::Vanilla, c.hide_shulker_box_lore);
    frame_lock(game, &c);
    let stale = {
        let cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        cache.atlas.as_ref() != Some(&c.colors)
    };
    if !stale {
        return;
    }
    use super::shulkerboxtooltip_texture as tex;
    let (tw, th) = (tex::WIDTH, tex::HEIGHT);
    let rows = (COLORS.len() as u32).div_ceil(ATLAS_COLS);
    let (w, h) = (ATLAS_COLS * tw, rows * th);
    let mut img = vec![0u8; (w * h * 4) as usize];
    for (i, color) in c.colors.iter().enumerate() {
        let [cx, cy] = atlas_cell(i);
        let tint = [(color >> 16) & 0xFF, (color >> 8) & 0xFF, color & 0xFF];
        for y in 0..th {
            for x in 0..tw {
                let s = ((y * tw + x) * 4) as usize;
                let d = (((cy as u32 + y) * w + cx as u32 + x) * 4) as usize;
                for ch in 0..3 {
                    img[d + ch] = ((u32::from(tex::RGBA[s + ch]) * tint[ch] + 127) / 255) as u8;
                }
                img[d + 3] = tex::RGBA[s + 3];
            }
        }
    }
    if game.native_mod_image(w, h, &img) {
        CACHE.lock().unwrap_or_else(|e| e.into_inner()).atlas = Some(c.colors.clone());
    }
}

const MOD_IMAGE: &str = "horizon/modimage";

/// `GuiGraphics.blitSprite` for a nine-slice sprite with `stretch_inner: false`: corners as
/// they are, edges and centre tiled from their start (the last tile cut).
fn nine_slice(game: &super::Game<'_>, batch: &mut HudBatch, cell: [f32; 2], x: f32, y: f32, w: f32, h: f32) {
    use super::shulkerboxtooltip_texture as tex;
    let (tw, th, b) = (tex::WIDTH as f32, tex::HEIGHT as f32, tex::BORDER as f32);
    let (iw, ih) = (tw - 2.0 * b, th - 2.0 * b);
    let tile = |batch: &mut HudBatch, sx: f32, sy: f32, sw: f32, sh: f32, dx: f32, dy: f32, dw: f32, dh: f32| {
        let mut yy = 0.0;
        while yy < dh {
            let ch = sh.min(dh - yy);
            let mut xx = 0.0;
            while xx < dw {
                let cw = sw.min(dw - xx);
                game.native_sprite(batch, MOD_IMAGE, [cell[0] + sx, cell[1] + sy, cw, ch], [dx + xx, dy + yy, cw, ch]);
                xx += sw;
            }
            yy += sh;
        }
    };
    let (mw, mh) = (w - 2.0 * b, h - 2.0 * b);
    // corners
    tile(batch, 0.0, 0.0, b, b, x, y, b, b);
    tile(batch, tw - b, 0.0, b, b, x + w - b, y, b, b);
    tile(batch, 0.0, th - b, b, b, x, y + h - b, b, b);
    tile(batch, tw - b, th - b, b, b, x + w - b, y + h - b, b, b);
    // edges
    tile(batch, b, 0.0, iw, b, x + b, y, mw, b);
    tile(batch, b, th - b, iw, b, x + b, y + h - b, mw, b);
    tile(batch, 0.0, b, b, ih, x, y + b, b, mh);
    tile(batch, tw - b, b, b, ih, x + w - b, y + b, b, mh);
    // centre
    tile(batch, b, b, iw, ih, x + b, y + b, mw, mh);
}

/// A vanilla GUI sprite drawn whole into `dst`.
fn sprite(game: &super::Game<'_>, batch: &mut HudBatch, name: &str, dst: [f32; 4]) {
    if let Some([w, h]) = game.native_sprite_size(name) {
        game.native_sprite(batch, name, [0.0, 0.0, w, h], dst);
    }
}

/// `BasePreviewRenderer.drawItem`: the item, its decorations and our own count label.
fn draw_item(game: &super::Game<'_>, batch: &mut HudBatch, stack: &ItemStack, x: f32, y: f32, short: bool) {
    game.native_item_stack(batch, stack, [x, y], 16.0);
    if stack.count != 1 {
        let label = if short { abbreviate_integer(i64::from(stack.count)) } else { stack.count.to_string() };
        // GuiGraphics.itemDecorations: x + 19 - 2 - width, y + 6 + 3, white with shadow.
        let w = game.native_text_width(&label);
        game.native_text(batch, &label, [x + 19.0 - 2.0 - w, y + 6.0 + 3.0], 0xFFFFFF, true);
    }
}

/// `BasePreviewRenderer.getSlotAt`.
fn slot_at(pv: &Preview, x: i32, y: i32, slot_w: i32, slot_h: i32, off_x: i32, off_y: i32, row: i32) -> i32 {
    if x + 1 >= off_x && y + 1 >= off_y {
        let sx = (x + 1 - off_x) / slot_w;
        let sy = (y + 1 - off_y) / slot_h;
        if sx < row {
            return sx + sy * row;
        }
    }
    let _ = pv;
    -1
}

/// `BasePreviewRenderer.getStackAt`, recorded for the lock's inner tooltip.
fn record_inner(pv: &Preview, slot: i32) {
    if slot < 0 || locked_slot().is_none() {
        return;
    }
    let s = slot as usize;
    let stack = if pv.kind == PreviewType::Compact {
        pv.compact.get(s).map(|m| m.stack.clone())
    } else {
        pv.full.get(s).cloned().flatten()
    };
    if let Some(stack) = stack {
        INNER.with(|i| *i.borrow_mut() = Some(stack));
    }
}

/// `PreviewClientTooltipComponent.extractImage` + the renderer's `draw`.
pub fn draw(game: &super::Game<'_>, stack: &ItemStack, batch: &mut HudBatch, origin: [f32; 2], size: [f32; 2]) {
    let Some(pv) = Preview::of(game, stack) else {
        return;
    };
    if pv.compact.is_empty() || pv.c.position != Position::Inside {
        return;
    }
    let mouse = game.native_cursor();
    match pv.c.theme {
        Theme::ShulkerBoxTooltip => draw_mod(game, &pv, batch, origin[0], origin[1], mouse),
        Theme::Vanilla => {
            // VanillaPreviewRenderer.draw: centred in the viewport (the component's width here).
            let x = origin[0] + ((size[0] as i32 - pv.width()) / 2) as f32;
            draw_vanilla(game, &pv, batch, x, origin[1], mouse);
        }
    }
}

/// `ModPreviewRenderer.draw`.
fn draw_mod(game: &super::Game<'_>, pv: &Preview, batch: &mut HudBatch, x: f32, y: f32, mouse: [f32; 2]) {
    let row = pv.row_size();
    let inv = pv.inv_size();
    let columns = row.min(inv);
    let rows = ceil_div(inv, row);
    // drawBackground
    nine_slice(game, batch, atlas_cell(if pv.c.colored_preview { pv.p.color } else { 0 }), x, y, (14 + columns * 18) as f32, (14 + rows * 18) as f32);
    let _ = pv.color();
    let hovered = slot_at(pv, (mouse[0] - x) as i32, (mouse[1] - y) as i32, 18, 18, 8, 8, row);
    record_inner(pv, hovered);
    let slot_xy = |slot: i32| (8.0 + x + 18.0 * (slot % row) as f32, 8.0 + y + 18.0 * (slot / row) as f32);
    let draw_slot = |batch: &mut HudBatch, stack: Option<&ItemStack>, slot: i32, short: bool| {
        let (sx, sy) = slot_xy(slot);
        let highlighted = hovered == slot;
        if highlighted {
            sprite(game, batch, "gui/sprites/container/slot_highlight_back", [sx - 4.0, sy - 4.0, 24.0, 24.0]);
        }
        if let Some(s) = stack {
            draw_item(game, batch, s, sx, sy, short);
        }
        if highlighted {
            sprite(game, batch, "gui/sprites/container/slot_highlight_front", [sx - 4.0, sy - 4.0, 24.0, 24.0]);
        }
    };
    if pv.kind == PreviewType::Full {
        // drawInactiveSlots: active slot count == inventory size for every provider here.
        for (slot, s) in pv.full.iter().enumerate() {
            draw_slot(batch, s.as_ref(), slot as i32, false);
        }
    } else {
        for (slot, m) in pv.compact.iter().enumerate() {
            draw_slot(batch, Some(&m.stack), slot as i32, pv.c.short_item_counts);
        }
    }
}

/// `VanillaPreviewRenderer.draw` (inside the tooltip: no background of its own).
fn draw_vanilla(game: &super::Game<'_>, pv: &Preview, batch: &mut HudBatch, x: f32, y: f32, mouse: [f32; 2]) {
    let row = pv.vanilla_row_size();
    // getSlotAt(x - 1, y - 1) with 24 px slots and no offset
    let hovered = slot_at(pv, (mouse[0] - x) as i32 - 1, (mouse[1] - y) as i32 - 1, 24, 24, 0, 0, row);
    record_inner(pv, hovered);
    let draw_slot = |batch: &mut HudBatch, stack: Option<&ItemStack>, slot: i32, short: bool| {
        let sx = x + 24.0 * (slot % row) as f32;
        let sy = y + 24.0 * (slot / row) as f32;
        let highlighted = hovered == slot;
        let back = if highlighted { "gui/sprites/container/bundle/slot_highlight_back" } else { "gui/sprites/container/bundle/slot_background" };
        sprite(game, batch, back, [sx, sy, 24.0, 24.0]);
        if let Some(s) = stack {
            draw_item(game, batch, s, sx + 4.0, sy + 4.0, short);
        }
        if highlighted {
            sprite(game, batch, "gui/sprites/container/bundle/slot_highlight_front", [sx, sy, 24.0, 24.0]);
        }
    };
    if pv.kind == PreviewType::Full {
        let last = pv.vanilla_inv_size() - 1;
        for (slot, s) in pv.full.iter().enumerate() {
            if slot as i32 <= last {
                draw_slot(batch, s.as_ref(), slot as i32, false);
            }
        }
    } else {
        for (slot, m) in pv.compact.iter().enumerate() {
            draw_slot(batch, Some(&m.stack), slot as i32, pv.c.short_item_counts);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abbreviates_like_java() {
        assert_eq!(abbreviate_integer(999), "999");
        assert_eq!(abbreviate_integer(1000), "1k");
        assert_eq!(abbreviate_integer(1234), "1.2k");
        assert_eq!(abbreviate_integer(12345), "12k");
        assert_eq!(abbreviate_integer(1_500_000), "1.5M");
        assert_eq!(abbreviate_integer(-2500), "-2.5k");
    }

    #[test]
    fn dye_defaults_clamp() {
        let black = &COLORS[color_index("black_shulker_box")];
        assert_eq!(black.default_rgb(), 0x262626);
        assert_eq!(COLORS[color_index("shulker_box")].default_rgb(), 0x976797);
    }

    #[test]
    fn config_round_trips() {
        let mut c = Config::default();
        c.position = Position::OutsideTop;
        c.colors[5] = 0x123456;
        let e = c.entries();
        let back = Config::load(&|k| e.iter().find(|(kk, _)| kk == k).map(|(_, v)| v.clone()));
        assert_eq!(back, c);
    }
}
