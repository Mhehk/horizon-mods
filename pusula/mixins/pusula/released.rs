// The button up on the world map: the drag ends.
fn pusula_released(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    use crate::game::remote::pusula as m;
    let _ = (screen, button);
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    *canceled = true;
    m::released();
}