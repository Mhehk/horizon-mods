// The mod's keys (held and clicked).
fn healthindicators_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::remote::healthindicators::key(game, key, pressed);
}
