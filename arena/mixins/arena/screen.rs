// Draws the HUD editor: the modules where they are, and the editor over them.
fn arena_screen(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
) {
    use crate::game::remote::arena as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    m::screen(game, batch, screen.mouse);
}
