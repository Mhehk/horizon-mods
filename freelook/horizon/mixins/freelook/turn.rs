// The mouse turns the camera while free looking (EntityMixin).
fn freelook_turn(xo: f64, yo: f64) -> core::ops::ControlFlow<()> {
    if crate::game::perspective::freelook::turn(xo, yo) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
