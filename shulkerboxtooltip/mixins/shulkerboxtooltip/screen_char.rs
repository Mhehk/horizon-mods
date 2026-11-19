// Typing into the configuration screen's text fields.
fn shulkerboxtooltip_screen_char(game: &crate::game::Game<'_>, c: char, canceled: &mut bool) {
    if crate::game::shulkerboxtooltip_screen::is_open(game) {
        crate::game::shulkerboxtooltip_screen::typed(c);
        *canceled = true;
    }
}
