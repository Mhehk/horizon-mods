// Every packet the server sends: the late copy of a hit already shown is skipped.
fn betterhitreg_packet(game: &mut crate::game::Game<'_>, packet: &C) -> core::ops::ControlFlow<Result<()>> {
    if crate::game::remote::betterhitreg::skip_packet(game, packet) {
        return core::ops::ControlFlow::Break(Ok(()));
    }
    core::ops::ControlFlow::Continue(())
}
