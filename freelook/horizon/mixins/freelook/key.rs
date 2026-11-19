// Freelook's keys going down and up.
fn freelook_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::perspective::freelook::key(game, key, pressed);
}
