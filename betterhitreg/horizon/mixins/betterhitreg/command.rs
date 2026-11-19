// /hitreg: the menu, a toggle, or setHitreg / setMetronome.
fn betterhitreg_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    if !*ran && crate::game::remote::betterhitreg_config::command(game, command) {
        *ran = true;
    }
}
