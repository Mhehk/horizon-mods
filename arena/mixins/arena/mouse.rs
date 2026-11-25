// A button down on the HUD editor: the list toggles, a box is taken or reset.
fn arena_mouse(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    use crate::game::remote::arena as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    *canceled = true;
    m::press(game, screen.mouse, button);
}
