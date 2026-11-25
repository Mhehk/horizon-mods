// Keys on the HUD editor: its own key closes it, Escape is the game's.
fn arena_keys(game: &crate::game::Game<'_>, key: &str, modifiers: u32, canceled: &mut bool) {
    use crate::game::remote::arena as m;
    let _ = modifiers;
    if game.native_mod_screen() != Some(m::ID) || key == "key.keyboard.escape" {
        return;
    }
    *canceled = true;
    if game.native_mod_key(m::ID, m::KEYS[0].0, m::KEYS[0].2) == key {
        m::state().editor.close_asked = true;
    }
}
