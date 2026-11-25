// Each tick: Toggle Sprint keeps you sprinting while it is on.
fn arena_tick(game: &mut crate::game::Game<'_>) {
    if !game.native_window() {
        return;
    }
    crate::game::remote::arena::tick(game);
}
