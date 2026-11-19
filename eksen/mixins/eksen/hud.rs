// The editor's box in the top right corner, while there is something to show.
fn eksen_hud(game: &mut crate::game::Game<'_>, batch: &mut HudBatch, corners: &mut Corners) {
    use crate::game::eksen as m;
    if !game.native_on(m::ID) {
        return;
    }
    let lines = m::state().hud_lines();
    if lines.is_empty() || !game.native_corner_shown(Corner::TopRight) {
        return;
    }
    let w = lines
        .iter()
        .map(|l| game.native_text_width(l))
        .fold(0.0f32, f32::max)
        + 6.0;
    let h = lines.len() as f32 * 10.0 + 4.0;
    let (x, y) = corners.place(Corner::TopRight, w, h);
    game.native_fill(batch, [x, y, w, h], 0x78000000);
    for (i, line) in lines.iter().enumerate() {
        let rgb = if i == 0 { 0x55FFFF } else { 0xFFFFFF };
        game.native_text(batch, line, [x + 3.0, y + 3.0 + i as f32 * 10.0], rgb, true);
    }
}
