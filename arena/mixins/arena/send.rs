// Every packet this client sends: an attack starts Reach and Combo.
fn arena_send(game: &crate::game::Game<'_>, packet: &S) {
    crate::game::remote::arena::on_send(game, packet);
}
