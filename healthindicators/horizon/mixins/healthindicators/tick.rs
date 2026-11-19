// Each client tick: the keys, the tracker and the damage direction.
fn healthindicators_tick(game: &mut crate::game::Game<'_>) {
    crate::game::remote::healthindicators::tick(game);
}
