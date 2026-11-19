// The player's own body shows while the camera is elsewhere (WorldRendererMixin).
fn ozgurkamera_body() -> core::ops::ControlFlow<bool> {
    if crate::game::perspective::ozgurkamera::active() {
        return core::ops::ControlFlow::Break(true);
    }
    core::ops::ControlFlow::Continue(())
}
