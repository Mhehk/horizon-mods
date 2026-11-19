// Each frame: the hits decided since the last one play at once; Debug Inputs reads the keys.
fn betterhitreg_frame(game: &mut crate::game::Game<'_>) {
    crate::game::remote::betterhitreg::on_frame(game);
    crate::game::remote::betterhitreg_fight::frame(game);
}
