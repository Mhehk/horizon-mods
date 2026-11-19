// A button of Zoomify's settings: a category, a value, a preset, Undo, Reset or Done.
fn zoomify_settings_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id == crate::game::fov::zoomify::ID {
        crate::game::fov::zoomify::click(game, button);
    }
}
