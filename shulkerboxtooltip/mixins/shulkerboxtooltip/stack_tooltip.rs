// AbstractContainerScreenMixin.lockTooltipPosition: while the lock key is held, the
// screens' stack tooltip is drawn at the locked slot's position, with the hovered preview
// item's own tooltip over it.
fn shulkerboxtooltip_stack_tooltip(
    game: &mut crate::game::Game<'_>,
    batch: &mut HudBatch,
    stack: &ItemStack,
    creative: bool,
    x: f32,
    y: f32,
    after_name: &Vec<(horizon_client::text::Text, u32)>,
) -> core::ops::ControlFlow<()> {
    use crate::game::shulkerboxtooltip as m;
    if game.native_on(m::ID) && m::tooltip_head(game, batch, stack, creative, x, y, after_name) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
