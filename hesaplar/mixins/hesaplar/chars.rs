// Characters typed on the account menu go into the client id field.
fn hesaplar_chars(game: &crate::game::Game<'_>, c: char, canceled: &mut bool) {
    use crate::game::hesaplar as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    if m::char(c) {
        *canceled = true;
    }
}