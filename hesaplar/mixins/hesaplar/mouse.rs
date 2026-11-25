// A left click on a menu screen: the account button, or the open menu.
fn hesaplar_mouse(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    use crate::game::hesaplar as m;
    let ours = game.native_mod_screen() == Some(m::ID);
    if button == 0 && m::press(game, screen.mouse) {
        *canceled = true;
    } else if ours {
        *canceled = true;
    }
}