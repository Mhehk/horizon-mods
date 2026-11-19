// Each frame: leaving the settings page goes back to its category list.
fn zoomify_frame(game: &mut crate::game::Game<'_>) {
    crate::game::fov::zoomify::frame(game);
}
