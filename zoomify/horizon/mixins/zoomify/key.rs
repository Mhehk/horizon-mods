// Zoomify's keys going down and up.
fn zoomify_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::fov::zoomify::key(game, key, pressed);
}
