// The wheel over the configuration screen scrolls its list.
fn shulkerboxtooltip_screen_scroll(
    game: &crate::game::Game<'_>,
    screen: &NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    if crate::game::shulkerboxtooltip_screen::is_open(game) {
        crate::game::shulkerboxtooltip_screen::scrolled(screen.mouse, scroll[1]);
        *canceled = true;
    }
}
