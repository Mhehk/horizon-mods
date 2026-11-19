// The scroll wheel zooms while zooming (MouseMixin.scrollStepCounter).
fn zoomify_scroll(game: &crate::game::Game<'_>, accumulated: [i32; 2], canceled: &mut bool) {
    crate::game::fov::zoomify::scroll(game, accumulated, canceled);
}
