// DamageMixin: Mute Hits: Them takes away your own hurt sound.
fn betterhitreg_hurt(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<&'static str> {
    if game.native_window()
        && game.native_on(crate::game::remote::betterhitreg::ID)
        && crate::game::remote::betterhitreg_config::on("silenceThem")
    {
        return core::ops::ControlFlow::Break("");
    }
    core::ops::ControlFlow::Continue(())
}
