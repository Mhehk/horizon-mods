// A button of the config screen.
fn healthindicators_settings_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id == crate::game::remote::healthindicators::ID {
        crate::game::remote::healthindicators::click(game, button);
    }
}
