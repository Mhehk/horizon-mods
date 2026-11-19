// Keys over the configuration screen: key capture, text fields, Escape (its own close).
fn shulkerboxtooltip_screen_key(
    game: &crate::game::Game<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    if crate::game::shulkerboxtooltip_screen::is_open(game) {
        crate::game::shulkerboxtooltip_screen::key(key, modifiers);
        *canceled = true;
    }
}
