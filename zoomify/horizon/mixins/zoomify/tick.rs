// Zoomify.tick at the end of each client tick.
fn zoomify_tick(game: &mut crate::game::Game<'_>) {
    crate::game::fov::zoomify::tick(game);
}
