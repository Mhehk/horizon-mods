// No HUD during the secondary zoom (secondary HudMixin).
fn zoomify_hud_layer(game: &crate::game::Game<'_>, canceled: &mut bool) {
    crate::game::fov::zoomify::hud_layer(game, canceled);
}
