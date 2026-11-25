// Keys on the editor screen: a field being typed in takes them (Enter sends,
// Escape stops typing); Right Shift closes the screen, Escape is left to the
// game (it closes it too); Ctrl+Z / Y / R / F are the editor's shortcuts;
// Magic Select's key selects from the block under the pointer.
fn eksen_ui_keys(game: &crate::game::Game<'_>, key: &str, modifiers: u32, canceled: &mut bool) {
    use crate::game::eksen as m;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    let (took, command) = m::state().ui().field_key(key);
    if took {
        *canceled = true;
        if let Some(words) = command {
            let lines = m::run_from_screen(game, &words);
            m::state().ui().say(lines);
        }
        return;
    }
    let bound = |i: usize| game.native_mod_key(m::ID, m::KEYS[i].0, m::KEYS[i].2) == key;
    if bound(m::KEY_EDITOR) {
        *canceled = true;
        m::state().ui().close_asked = true;
        return;
    }
    if key == "key.keyboard.escape" {
        return;
    }
    *canceled = true;
    // GLFW's modifier bits: 1 shift, 2 control.
    if modifiers & 2 != 0 {
        let sub = match key {
            "key.keyboard.z" => "geri",
            "key.keyboard.y" => "ileri",
            "key.keyboard.r" => "cevir",
            "key.keyboard.f" => "aynala",
            _ => return,
        };
        let lines = m::run_from_screen(game, &[sub.to_owned()]);
        m::state().ui().say(lines);
        return;
    }
    if bound(m::KEY_MAGIC) {
        let line = {
            let mut s = m::state();
            let hover = s.ui().hover;
            let Some((at, _)) = hover else {
                s.ui().say(vec![format!("{}İmlecin altında blok yok.", m::PREFIX)]);
                return;
            };
            s.magic_seeds = vec![at];
            s.finish_magic(&m::GameBlocks(game))
        };
        m::state().ui().say(vec![line]);
    }
}
