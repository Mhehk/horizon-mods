// The spyglass's stop sound with Zoomify's Sound Behavior (SpyglassItemMixin).
fn zoomify_stop_spyglass(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    crate::game::fov::zoomify::stop_spyglass(game)
}
