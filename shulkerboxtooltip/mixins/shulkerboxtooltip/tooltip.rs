// ItemStackMixin.onGetTooltip: the mod's lines at the end of the tooltip.
fn shulkerboxtooltip_tooltip(game: &crate::game::Game<'_>, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    if game.native_on(crate::game::shulkerboxtooltip::ID) {
        crate::game::shulkerboxtooltip::tooltip_lines(game, stack, lines);
    }
}
