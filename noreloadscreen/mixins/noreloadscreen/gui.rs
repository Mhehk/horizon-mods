// After the whole GUI: the hidden overlay's mini version (RRLS's GuiMixin), or
// the settings section's preview overlay.
fn noreloadscreen_gui(game: &crate::game::Game<'_>, batch: &mut HudBatch) {
    crate::game::packs::noreloadscreen::draw(game, batch);
}
