// /zoomify opens the settings.
fn zoomify_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    if !*ran && crate::game::fov::zoomify::command(game, command) {
        *ran = true;
    }
}
