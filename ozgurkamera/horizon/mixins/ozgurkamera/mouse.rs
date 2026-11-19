// No attack, use, pick block or block breaking while the camera flies (MinecraftClientMixin).
fn ozgurkamera_mouse(pressed: bool) -> core::ops::ControlFlow<Result<()>> {
    if pressed && crate::game::perspective::ozgurkamera::active() {
        return core::ops::ControlFlow::Break(Ok(()));
    }
    core::ops::ControlFlow::Continue(())
}
