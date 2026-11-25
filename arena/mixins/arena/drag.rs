// The pointer dragged on the HUD editor: the module held follows it.
fn arena_drag(
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
    if button == 0 {
        m::drag(game, screen.mouse);
    }
}
