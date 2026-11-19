// The spyglass's use sound with Zoomify's Sound Behavior (SpyglassItemMixin).
fn zoomify_use_spyglass(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    crate::game::fov::zoomify::use_spyglass(game)
}
