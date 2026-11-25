// Each frame: the editor screen's state follows the game (Escape or another
// screen closed it), it closes when asked (Right Shift), and what its
// commands answered goes to the chat.
fn eksen_ui_frame(game: &mut crate::game::Game<'_>) {
    use crate::game::eksen as m;
    let open = game.native_mod_screen() == Some(m::ID);
    let (close, chat) = {
        let mut s = m::state();
        let closed = !open && s.ui.as_ref().is_some_and(|u| u.open);
        if closed {
            // A stroke cut off by the screen closing is dropped.
            let held = s.ui().world_button.take().is_some();
            if held {
                let e = s.editor();
                e.held = false;
                e.stroke.clear();
            }
            let u = s.ui();
            u.field = None;
            u.hover = None;
        }
        let u = s.ui();
        u.open = open;
        (std::mem::take(&mut u.close_asked), std::mem::take(&mut u.chat))
    };
    if close && open {
        game.native_mod_screen_close(m::ID);
    }
    for line in chat {
        game.native_chat(&line);
    }
}
