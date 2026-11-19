// A button of the configuration page: open a section, change a value, Undo, Reset or Done.
fn noreloadscreen_settings_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id == crate::game::packs::noreloadscreen::ID {
        crate::game::packs::noreloadscreen::click(game, button);
    }
}
