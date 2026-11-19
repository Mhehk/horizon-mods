// Every packet this client sends: an attack is decided and queued here.
fn betterhitreg_send(game: &crate::game::Game<'_>, packet: &S) {
    crate::game::remote::betterhitreg::on_send(game, packet);
}
