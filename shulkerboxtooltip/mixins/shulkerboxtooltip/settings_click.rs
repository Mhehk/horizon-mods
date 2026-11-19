// The configuration screen's button was clicked: the screen opens over the world.
fn shulkerboxtooltip_settings_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id == crate::game::shulkerboxtooltip::ID && button == 0 {
        crate::game::shulkerboxtooltip_screen::open(game);
    }
}
