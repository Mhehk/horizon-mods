// The selection, its corners and the brush's reach, drawn in the world.
fn eksen_world(game: &crate::game::Game<'_>, primitives: &mut WorldPrimitives) {
    use crate::game::eksen as m;
    if !game.native_on(m::ID) {
        return;
    }
    let (pos1, pos2, brush, set) = {
        let s = m::state();
        let set = if s.set.len() <= m::DRAW_MAX { Ok(s.set.iter().copied().collect::<Vec<_>>()) } else { Err(crate::game::eksen_magic::bounds(&s.set)) };
        (s.pos1, s.pos2, s.brush.as_ref().map(|b| b.radius), set)
    };
    // A Magic Select set: each block, or past `DRAW_MAX` the box around them.
    match set {
        Ok(blocks) => {
            for p in blocks {
                let lo = [p[0] as f64 - 0.005, p[1] as f64 - 0.005, p[2] as f64 - 0.005];
                let hi = [p[0] as f64 + 1.005, p[1] as f64 + 1.005, p[2] as f64 + 1.005];
                primitives.filled_box(lo, hi, 0x2055FFFF);
            }
        }
        Err(Some((a, b))) => {
            let lo = [a[0] as f64 - 0.01, a[1] as f64 - 0.01, a[2] as f64 - 0.01];
            let hi = [b[0] as f64 + 1.01, b[1] as f64 + 1.01, b[2] as f64 + 1.01];
            primitives.box_outline(lo, hi, 0xFF55FFFF, 2.0);
        }
        Err(None) => {}
    }
    let block = |p: m::Pos, grow: f64| {
        (
            [p[0] as f64 - grow, p[1] as f64 - grow, p[2] as f64 - grow],
            [p[0] as f64 + 1.0 + grow, p[1] as f64 + 1.0 + grow, p[2] as f64 + 1.0 + grow],
        )
    };
    if let (Some(a), Some(b)) = (pos1, pos2) {
        let r = m::Region::new(a, b);
        let (lo, hi) = r.bounds();
        let lo = lo.map(|v| v - 0.01);
        let hi = hi.map(|v| v + 0.01);
        primitives.filled_box(lo, hi, 0x1455FFFF);
        primitives.box_outline(lo, hi, 0xFF55FFFF, 2.0);
        primitives.box_outline(lo, hi, 0x5555FFFF, 1.0).through_walls();
        let s = r.size();
        let top = [(lo[0] + hi[0]) / 2.0, hi[1] + 0.6, (lo[2] + hi[2]) / 2.0];
        primitives
            .text(top, format!("{}x{}x{}", s[0], s[1], s[2]), 0xFFFFFFFF, 1.0)
            .scale_with_distance()
            .background();
    }
    if let Some(a) = pos1 {
        let (lo, hi) = block(a, 0.02);
        primitives.box_outline(lo, hi, 0xFFFF5555, 3.0).through_walls();
    }
    if let Some(b) = pos2 {
        let (lo, hi) = block(b, 0.02);
        primitives.box_outline(lo, hi, 0xFF5599FF, 3.0).through_walls();
    }
    // The editor screen: the block under the pointer and the stroke drawn.
    let (hover, stroke) = {
        let mut s = m::state();
        let open = s.ui.as_ref().is_some_and(|u| u.open);
        let hover = if open { s.ui().hover } else { None };
        let e = s.editor();
        let stroke: Vec<m::Pos> = if open && e.held { e.stroke.iter().take(m::DRAW_MAX).copied().collect() } else { Vec::new() };
        (hover, stroke)
    };
    if let Some((p, _)) = hover {
        let (lo, hi) = block(p, 0.01);
        primitives.box_outline(lo, hi, 0xFFFFFFFF, 2.0);
    }
    for p in stroke {
        let (lo, hi) = block(p, 0.005);
        primitives.filled_box(lo, hi, 0x30FFD040);
    }
    if let Some(r) = brush {
        if let Some(hit) = game.native_raycast(m::REACH) {
            let c = [hit.block_x, hit.block_y, hit.block_z];
            let (lo, hi) = block(c, r as f64);
            primitives.box_outline(lo, hi, 0xAA55FF55, 1.5);
        }
    }
}
