// The pointer moving with a button held in the world of the editor screen:
// each new block under it joins the active tool's stroke.
fn eksen_ui_drag(
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
        s.ui().size
    };
    let Some((Some((at, _)), _, _)) = m::pointer(game, screen.mouse, size) else {
        return;
    };
    let mut s = m::state();
    let e = s.editor();
    if e.held && e.stroke.len() < 4096 && !e.stroke.contains(&at) {
        e.stroke.push(at);
    }
}
