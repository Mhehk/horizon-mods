// The toggle key (FreeCamera.TOGGLE).
fn ozgurkamera_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::perspective::ozgurkamera::key(game, key, pressed);
}
