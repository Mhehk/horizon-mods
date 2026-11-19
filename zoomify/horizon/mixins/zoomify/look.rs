// Relative sensitivity and the cinematic camera while zooming (MouseMixin).
fn zoomify_look(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<LookOptions> {
    crate::game::fov::zoomify::look(game)
}
