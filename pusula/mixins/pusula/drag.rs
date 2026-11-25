// The pointer dragged on the world map: the map follows it.
fn pusula_drag(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    use crate::game::remote::pusula as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    *canceled = true;
    if button == 0 {
        m::drag(screen.mouse);
    }
}