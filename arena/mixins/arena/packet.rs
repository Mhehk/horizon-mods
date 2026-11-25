// Every packet the server sends: damage events confirm hits (or end the
// combo when they are on you). Nothing is cancelled.
fn arena_packet(game: &mut crate::game::Game<'_>, packet: &C) -> core::ops::ControlFlow<Result<()>> {
    crate::game::remote::arena::on_packet(game, packet);
    core::ops::ControlFlow::Continue(())
}
