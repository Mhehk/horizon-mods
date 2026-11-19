// Each frame: leaving the settings page resets its sub-page.
fn freelook_frame(game: &mut crate::game::Game<'_>) {
    crate::game::perspective::freelook::frame(game);
}
