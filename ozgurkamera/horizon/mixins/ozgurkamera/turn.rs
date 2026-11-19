// The mouse turns the camera, not the player (MouseMixin).
fn ozgurkamera_turn(xo: f64, yo: f64) -> core::ops::ControlFlow<()> {
    if crate::game::perspective::ozgurkamera::turn(xo, yo) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
