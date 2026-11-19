// Drop and swap-hands presses are drained while the camera flies (FreeCamera.drainActions).
fn ozgurkamera_keys(key: KeyCode, pressed: bool) -> core::ops::ControlFlow<Result<()>> {
    if pressed
        && matches!(key, KeyCode::KeyQ | KeyCode::KeyF)
        && crate::game::perspective::ozgurkamera::active()
    {
        return core::ops::ControlFlow::Break(Ok(()));
    }
    core::ops::ControlFlow::Continue(())
}
