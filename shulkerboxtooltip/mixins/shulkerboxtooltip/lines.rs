// ItemContainerContentsMixin.addToTooltip: the vanilla "N x item" lines are left out
// unless tooltip.type is VANILLA.
fn shulkerboxtooltip_container_lines(stack: &ItemStack) -> core::ops::ControlFlow<Vec<String>> {
    let _ = stack;
    if crate::item_tooltip::shulkerboxtooltip_flags::hide_container_lines() {
        return core::ops::ControlFlow::Break(Vec::new());
    }
    core::ops::ControlFlow::Continue(())
}
