// The configuration screen, drawn over the mod's own screen.
fn shulkerboxtooltip_screen_draw(game: &crate::game::Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if crate::game::shulkerboxtooltip_screen::is_open(game) {
        crate::game::shulkerboxtooltip_screen::draw(game, batch, screen.mouse);
    }
}
