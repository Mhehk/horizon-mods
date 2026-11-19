// The monitored entity's health and the damage direction arc.
fn healthindicators_hud(game: &mut crate::game::Game<'_>, batch: &mut HudBatch, corners: &mut Corners) {
    let _ = corners;
    crate::game::remote::healthindicators::hud(game, batch);
}
