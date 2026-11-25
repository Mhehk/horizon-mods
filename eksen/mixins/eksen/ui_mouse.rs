// A mouse button on the editor screen: a button runs its command, a field
// takes the typing, and in the world (between the panels) the left button
// starts the active tool's stroke at the block under the pointer, the right
// one its second action.
fn eksen_ui_mouse(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    use crate::game::eksen as m;
    use crate::game::eksen_ui as ui;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    *canceled = true;
    let measure = |t: &str| game.native_text_width(t);
    let hit = {
        let mut s = m::state();
        let view = ui::View::of(&mut s);
        let u = s.ui();
        if button == 0 {
            u.click(&view, screen.mouse, &measure)
        } else {
            u.hit_at(&view, screen.mouse, &measure)
        }
    };
    match hit {
        ui::Hit::Run(words) if button == 0 => {
            let lines = m::run_from_screen(game, &words);
            m::state().ui().say(lines);
        }
        ui::Hit::World if button == 0 || button == 1 => {
            let size = m::state().ui().size;
            let Some((Some((at, face)), _, _)) = m::pointer(game, screen.mouse, size) else {
                m::state().ui().say(vec![format!("{}İmlecin altında blok yok.", m::PREFIX)]);
                return;
            };
            let mut s = m::state();
            s.ui().world_button = Some(button);
            crate::game::eksen_editor::start_stroke(&mut s, Some(at), face, button == 1);
        }
        _ => {}
    }
}
