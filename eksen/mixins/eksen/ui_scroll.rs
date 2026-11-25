// The wheel on the editor screen scrolls the left list.
fn eksen_ui_scroll(game: &crate::game::Game<'_>, scroll: [f64; 2], canceled: &mut bool) {
    use crate::game::eksen as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    *canceled = true;
    m::state().ui().scroll(scroll[1]);
}
