// Characters typed on the world map go into the new waypoint's name.
fn pusula_chars(game: &crate::game::Game<'_>, c: char, canceled: &mut bool) {
    use crate::game::remote::pusula as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    if m::char(c) {
        *canceled = true;
    }
}