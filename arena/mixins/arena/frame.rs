// Each frame: the editor follows the game (Escape closed it) and closes
// when its key asked.
fn arena_frame(game: &mut crate::game::Game<'_>) {
    use crate::game::remote::arena as m;
    let open = game.native_mod_screen() == Some(m::ID);
    let close = {
        let mut s = m::state();
        if !open {
            s.editor.release();
        }
        s.editor.open = open;
        std::mem::take(&mut s.editor.close_asked)
    };
    if close && open {
        game.native_mod_screen_close(m::ID);
    }
}
