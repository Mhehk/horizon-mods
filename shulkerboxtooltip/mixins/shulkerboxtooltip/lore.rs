// ItemStackMixin.removeLore: a shulker box's lore is left out when
// tooltip.hideShulkerBoxLore is on.
fn shulkerboxtooltip_lore(stack: &ItemStack) -> core::ops::ControlFlow<Vec<crate::text::Text>> {
    use crate::item_tooltip::shulkerboxtooltip_flags as f;
    if f::hide_shulker_lore() && f::is_shulker_box(&stack.item) {
        return core::ops::ControlFlow::Break(Vec::new());
    }
    core::ops::ControlFlow::Continue(())
}
