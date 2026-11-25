// Draws the editor screen (Right Shift) over the world, and finds the block
// under the pointer for the outline and the next click.
fn eksen_ui_screen(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
) {
    use crate::game::eksen as m;
    use crate::game::eksen_ui as ui;
    if game.native_mod_screen() != Some(m::ID) {
        return;
    }
    let size = [batch.width, batch.height];
    let hover = m::pointer(game, screen.mouse, size).and_then(|p| p.0);
    let draws = {
        let mut s = m::state();
        let view = ui::View::of(&mut s);
        let u = s.ui();
        u.size = size;
        u.hover = hover;
        u.draw(&view, screen.mouse, &|t| game.native_text_width(t))
    };
    for d in draws {
        match d {
            ui::Draw::Fill(rect, argb) => {
                game.native_fill(batch, rect, argb);
            }
            ui::Draw::Text(text, at, rgb, shadow) => {
                game.native_text(batch, &text, at, rgb, shadow);
            }
        }
    }
}
