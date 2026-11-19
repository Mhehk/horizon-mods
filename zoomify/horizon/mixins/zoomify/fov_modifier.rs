// The spyglass's own zoom unless combining (AbstractClientPlayerEntityMixin).
fn zoomify_fov_modifier(game: &crate::game::Game<'_>, player: &horizon_client::camera::FovPlayer, modifier: &mut f32) {
    crate::game::fov::zoomify::fov_modifier(game, player, modifier);
}
