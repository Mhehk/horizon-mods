// Each frame: the sign-in thread's news, a finished switch, the menu opened
// or closed as asked.
fn hesaplar_frame(game: &mut crate::game::Game<'_>) {
    crate::game::hesaplar::frame(game);
}