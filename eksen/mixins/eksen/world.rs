// The selection, its corners and the brush's reach, drawn in the world.
fn eksen_world(game: &crate::game::Game<'_>, primitives: &mut WorldPrimitives) {
    use crate::game::eksen as m;
    if !game.native_on(m::ID) {
        return;
    }
    let (pos1, pos2, brush) = {
        let s = m::state();
        (s.pos1, s.pos2, s.brush.as_ref().map(|b| b.radius))
    };
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
    if let Some(r) = brush {
        if let Some(hit) = game.native_raycast(m::REACH) {
            let c = [hit.block_x, hit.block_y, hit.block_z];
            let (lo, hi) = block(c, r as f64);
            primitives.box_outline(lo, hi, 0xAA55FF55, 1.5);
        }
    }
}
