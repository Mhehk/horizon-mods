// Each frame: the keys, clicks, frames and camera are read and the HUD
// modules drawn where they are placed (state.rs).
fn arena_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    let _ = corners;
    crate::game::remote::arena::hud(game, batch);
}
