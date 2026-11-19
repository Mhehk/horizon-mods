// Each client tick: the camera goes when its player, world or life does.
fn ozgurkamera_tick(game: &mut crate::game::Game<'_>) {
    crate::game::perspective::ozgurkamera::tick(game);
}
