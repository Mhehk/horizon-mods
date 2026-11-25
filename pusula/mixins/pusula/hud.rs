// The minimap, and the waypoints where they are in the world.
fn pusula_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    let _ = corners;
    crate::game::remote::pusula::hud(game, batch);
}