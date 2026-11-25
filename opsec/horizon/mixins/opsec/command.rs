// /opsec: how many packs OpSec answered on this server.
fn opsec_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    if !*ran && command.trim().eq_ignore_ascii_case("opsec") {
        game.native_chat(&crate::game::remote::opsec_guard::status());
        *ran = true;
    }
}
