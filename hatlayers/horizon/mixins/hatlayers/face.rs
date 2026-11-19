// `face` as the tab list draws a head (the skin's face, then its hat layer),
// plus the hat marker on the head when HatLayers decided this list's hats.
fn hatlayers_face(
    out: &mut Vec<GuiDraw>,
    skin: &str,
    x: i32,
    y: i32,
    size: i32,
    hat: bool,
    flip: bool,
    color: u32,
) -> core::ops::ControlFlow<()> {
    if !crate::tab_list::hatlayers_marker::active() {
        return core::ops::ControlFlow::Continue(());
    }
    let (v, height) = if flip { (16.0, -8) } else { (8.0, 8) };
    for u in [8.0, 40.0].into_iter().take(if hat { 2 } else { 1 }) {
        out.push(GuiDraw::Blit {
            texture: Cow::Owned(skin.to_owned()),
            blend: Blend::Textured,
            x,
            y,
            w: size,
            h: size,
            uv: [u, v],
            src: [8, height],
            tex: [64, 64],
            color,
        });
    }
    if hat {
        crate::tab_list::hatlayers_marker::draw(out, x, y, size, color);
    }
    core::ops::ControlFlow::Break(())
}
