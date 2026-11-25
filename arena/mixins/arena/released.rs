// The button up on the HUD editor: the module held stays where it is.
fn arena_released(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    use crate::game::remote::arena as m;
    let _ = (screen, button);
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    *canceled = true;
    m::state().editor.release();
}
