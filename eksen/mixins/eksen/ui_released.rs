// A button let go on the editor screen: a stroke made in the world runs the
// active tool, from the camera along the pointer's ray.
fn eksen_ui_released(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    use crate::game::eksen as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    *canceled = true;
    let size = {
        let mut s = m::state();
        if s.ui().world_button != Some(button) {
            return;
        }
        s.ui().world_button = None;
        s.ui().size
    };
    let (eye, look) = match m::pointer(game, screen.mouse, size) {
        Some((_, eye, look)) => (eye, look),
        None => ([0.0; 3], [0.0, 0.0, 1.0]),
    };
    let lines = {
        let world = m::GameBlocks(game);
        let mut s = m::state();
        crate::game::eksen_editor::finish_stroke(&mut s, &world, eye, look)
    };
    m::state().ui().say(lines);
}
