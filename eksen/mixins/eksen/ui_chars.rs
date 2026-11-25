// Characters typed on the editor screen go into the field being edited.
fn eksen_ui_chars(game: &crate::game::Game<'_>, c: char, canceled: &mut bool) {
    use crate::game::eksen as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    *canceled = true;
    m::state().ui().char(c);
}
