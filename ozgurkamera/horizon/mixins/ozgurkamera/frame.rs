// Each frame: the camera flies with the movement keys.
fn ozgurkamera_frame(game: &mut crate::game::Game<'_>) {
    crate::game::perspective::ozgurkamera::frame(game);
}
