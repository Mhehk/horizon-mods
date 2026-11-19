// Each frame: the config screen's requested close, the text crate's switches, and the window
// atlas when the colours changed.
fn shulkerboxtooltip_frame(game: &mut crate::game::Game<'_>) {
    if !game.native_window() {
        return;
    }
    crate::game::shulkerboxtooltip_screen::frame(game);
    if !game.native_on(crate::game::shulkerboxtooltip::ID) {
        horizon_client::item_tooltip::shulkerboxtooltip_flags::set(false, false);
        return;
    }
    crate::game::shulkerboxtooltip::frame(game);
}
