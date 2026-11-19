// Each frame: leaving the settings page goes back to its categories.
fn healthindicators_frame(game: &mut crate::game::Game<'_>) {
    crate::game::remote::healthindicators::frame(game);
}
