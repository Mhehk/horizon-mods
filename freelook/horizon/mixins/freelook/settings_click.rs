// A button of Freelook Config or Disabled Servers.
fn freelook_settings_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id == crate::game::perspective::freelook::ID {
        crate::game::perspective::freelook::click(game, button);
    }
}
