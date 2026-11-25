// Every menu screen out of a world: the account button in the top right,
// and the account menu under it while it is open.
fn hesaplar_screen(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
) {
    crate::game::hesaplar::screen(game, batch, screen.mouse);
}