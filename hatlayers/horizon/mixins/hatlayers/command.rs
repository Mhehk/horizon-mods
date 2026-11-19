// `/hatlayers [name]` and `/chunkhat [name]`: a local report, never sent to the
// server (ClientPlayNetworkHandlerMixin.sendChatCommand).
fn hatlayers_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    if !*ran && game.native_on("hatlayers") && crate::game::remote::hatlayers::handle(game, command) {
        *ran = true;
    }
}
