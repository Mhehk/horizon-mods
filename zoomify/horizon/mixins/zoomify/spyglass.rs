// The spyglass overlay with Zoomify's Overlay Visibility (spyglass HudMixin).
fn zoomify_spyglass(game: &mut crate::game::Game<'_>, batch: &mut HudBatch) -> core::ops::ControlFlow<()> {
    crate::game::fov::zoomify::spyglass_overlay(game, batch)
}
