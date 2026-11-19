// PreviewClientTooltipComponent.extractImage.
fn shulkerboxtooltip_draw(
    game: &crate::game::Game<'_>,
    stack: &ItemStack,
    batch: &mut HudBatch,
    origin: [f32; 2],
    size: [f32; 2],
) {
    if game.native_on(crate::game::shulkerboxtooltip::ID) {
        crate::game::shulkerboxtooltip::draw(game, stack, batch, origin, size);
    }
}
