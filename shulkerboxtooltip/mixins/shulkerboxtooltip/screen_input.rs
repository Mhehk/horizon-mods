// A mouse press over the configuration screen: ours, and no one else's.
fn shulkerboxtooltip_screen_press(
    game: &crate::game::Game<'_>,
    screen: &NativeScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) {
    if crate::game::shulkerboxtooltip_screen::is_open(game) {
        crate::game::shulkerboxtooltip_screen::press(game, screen.mouse, button, modifiers);
        *canceled = true;
    }
}
