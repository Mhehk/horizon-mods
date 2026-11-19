// A steady view: no view bobbing and no hurt tilt (GameRendererMixin).
fn ozgurkamera_bob() -> core::ops::ControlFlow<glam::Mat4> {
    if crate::game::perspective::ozgurkamera::active() {
        return core::ops::ControlFlow::Break(glam::Mat4::IDENTITY);
    }
    core::ops::ControlFlow::Continue(())
}
