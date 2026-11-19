// A button of BetterHitreg's menu.
fn betterhitreg_settings_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id == crate::game::remote::betterhitreg::ID {
        crate::game::remote::betterhitreg_config::click(game, button);
    }
}
