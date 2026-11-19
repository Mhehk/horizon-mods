// ItemStackMixin.onGetTooltipData + PreviewClientTooltipComponent.getWidth/getHeight.
fn shulkerboxtooltip_size(game: &crate::game::Game<'_>, stack: &ItemStack, size: &mut Option<[f32; 2]>) {
    if game.native_on(crate::game::shulkerboxtooltip::ID) {
        crate::game::shulkerboxtooltip::component_size(game, stack, size);
    }
}
