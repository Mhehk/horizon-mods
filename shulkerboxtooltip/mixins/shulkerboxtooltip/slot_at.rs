// AbstractContainerScreenMixin.forceFocusSlot: while a tooltip is locked, the screen
// hovers the locked slot wherever the mouse is.
fn shulkerboxtooltip_slot_at() -> core::ops::ControlFlow<Option<usize>> {
    if let Some(slot) = crate::game::shulkerboxtooltip::locked_slot() {
        return core::ops::ControlFlow::Break(Some(slot));
    }
    core::ops::ControlFlow::Continue(())
}
