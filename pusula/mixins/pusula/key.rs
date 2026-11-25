// Keys in the world: M opens the world map, B a new waypoint, = / - zoom.
fn pusula_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    use crate::game::remote::pusula as m;
    if !pressed || !game.native_window() || !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    m::key(game, key);
}