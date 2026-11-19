// Hat Layers — mhv1.2 (mihehk): the small outlined asterisk drawn on a TAB head
// whose hat layer shows (a player inside the active chunk view). TabHatMarker:
// a five-pixel asterisk at half scale with a one-pixel dark outline, kept in
// the head's top right so names and rows stay untouched at every GUI scale.
use std::sync::atomic::{AtomicBool, Ordering};

static ACTIVE: AtomicBool = AtomicBool::new(false);

const ROWS: [u32; 5] = [0b00100, 0b10101, 0b01110, 0b10101, 0b00100];

/// Whether this frame's list came from Hat Layers (a server's list, mod on).
pub fn set_active(on: bool) {
    ACTIVE.store(on, Ordering::Relaxed);
}
pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

fn fill(out: &mut Vec<super::GuiDraw>, x0: i32, y0: i32, x1: i32, y1: i32, color: u32) {
    out.push(super::GuiDraw::Fill { x0, y0, x1, y1, color });
}

/// `TabHatMarker.draw(context, headX, headY, headSize, headColor)`.
pub fn draw(out: &mut Vec<super::GuiDraw>, x: i32, y: i32, size: i32, color: u32) {
    use crate::hud_layout::PoseOp;
    let alpha = color & 0xFF00_0000;
    if alpha == 0 || size < 4 {
        return;
    }
    out.push(super::GuiDraw::Push(vec![
        PoseOp::Translate((x + size) as f32 - 3.0, y as f32 + 1.0),
        PoseOp::Scale(0.5, 0.5),
    ]));
    fill(out, 1, -1, 4, 6, alpha);
    fill(out, -1, 0, 6, 5, alpha);
    let white = alpha | 0x00FF_FFFF;
    for (row, bits) in ROWS.iter().enumerate() {
        let row = row as i32;
        let mut column = 0;
        while column < 5 {
            if bits & (1 << column) == 0 {
                column += 1;
                continue;
            }
            let start = column;
            column += 1;
            while column < 5 && bits & (1 << column) != 0 {
                column += 1;
            }
            fill(out, start, row, column, row + 1, white);
        }
    }
    out.push(super::GuiDraw::Pop);
}
