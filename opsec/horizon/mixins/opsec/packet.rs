// Every packet the server sends: a resource pack OpSec answers itself is not
// handled by Horizon (no download, no cache lookup).
fn opsec_packet(game: &mut crate::game::Game<'_>, packet: &C) -> core::ops::ControlFlow<Result<()>> {
    if crate::game::remote::opsec_guard::on_packet(game, packet) {
        return core::ops::ControlFlow::Break(Ok(()));
    }
    core::ops::ControlFlow::Continue(())
}
