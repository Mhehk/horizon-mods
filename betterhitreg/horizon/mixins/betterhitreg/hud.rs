// The score and Debug Inputs on the HUD.
fn betterhitreg_hud(game: &mut crate::game::Game<'_>, batch: &mut HudBatch, corners: &mut Corners) {
    let _ = corners;
    crate::game::remote::betterhitreg_fight::hud(game, batch);
}
