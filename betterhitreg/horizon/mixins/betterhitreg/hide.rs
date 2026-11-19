// RenderMixin: Hide Other Fights leaves other players near no fight of yours undrawn.
fn betterhitreg_hide(game: &crate::game::Game<'_>, id: u32) -> core::ops::ControlFlow<bool> {
    if crate::game::remote::betterhitreg_fight::hidden(game, id) {
        return core::ops::ControlFlow::Break(true);
    }
    core::ops::ControlFlow::Continue(())
}
