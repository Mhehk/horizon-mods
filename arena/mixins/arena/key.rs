// Right Alt opens the HUD editor (closing it is the screen's key hook).
fn arena_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    use crate::game::remote::arena as m;
    if !pressed || !game.native_window() || !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    if game.native_mod_key(m::ID, m::KEYS[0].0, m::KEYS[0].2) == key && game.native_mod_screen_open(m::ID) {
        m::state().editor.open = true;
    }
}
