// Keys on the world map: the dialog's, the map key closes it, = / - zoom.
// Escape is the game's (it closes the screen).
fn pusula_keys(game: &crate::game::Game<'_>, key: &str, modifiers: u32, canceled: &mut bool) {
    use crate::game::remote::pusula as m;
    let _ = modifiers;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    if m::screen_key(game, key) {
        *canceled = true;
    }
}