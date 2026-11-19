// ShulkerBoxTooltip (Horizon port, mihehk; original by MisterPeModder, MIT): the
// switches this crate's tooltip text code reads (`container_lines`, `lore_texts`),
// copied from the game crate's config every frame (they have no `Game`).
use std::sync::atomic::{AtomicBool, Ordering};

static HIDE_CONTAINER_LINES: AtomicBool = AtomicBool::new(false);
static HIDE_SHULKER_LORE: AtomicBool = AtomicBool::new(false);

/// `ItemContainerContentsMixin`: tooltip.type != VANILLA; `ItemStackMixin.removeLore`:
/// tooltip.hideShulkerBoxLore.
pub fn set(hide_container_lines: bool, hide_shulker_lore: bool) {
    HIDE_CONTAINER_LINES.store(hide_container_lines, Ordering::Relaxed);
    HIDE_SHULKER_LORE.store(hide_shulker_lore, Ordering::Relaxed);
}
pub fn hide_container_lines() -> bool {
    HIDE_CONTAINER_LINES.load(Ordering::Relaxed)
}
pub fn hide_shulker_lore() -> bool {
    HIDE_SHULKER_LORE.load(Ordering::Relaxed)
}
/// `item instanceof BlockItem && block instanceof ShulkerBoxBlock`.
pub fn is_shulker_box(item: &str) -> bool {
    let name = item.trim_start_matches("minecraft:");
    name == "shulker_box" || name.ends_with("_shulker_box")
}
