// Draws the world map when it is the screen open.
fn pusula_screen(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
) {
    use crate::game::remote::pusula as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    m::screen(game, batch, screen.mouse);
}