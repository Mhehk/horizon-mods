// Keys on the account menu: Backspace / Enter edit the client id; Escape is
// the game's and closes the menu.
fn hesaplar_keys(game: &crate::game::Game<'_>, key: &str, modifiers: u32, canceled: &mut bool) {
    use crate::game::hesaplar as m;
    let _ = modifiers;
    if game.native_mod_screen() != Some(m::ID) || key == "key.keyboard.escape" {
        return;
    }
    if m::key(game, key) {
        *canceled = true;
    }
}