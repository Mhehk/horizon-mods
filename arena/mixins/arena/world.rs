// Hitbox (a frame round each entity, its eye line) and Block Overlay (the
// looked-at block's frame and faces), drawn in the world.
fn arena_world(game: &crate::game::Game<'_>, primitives: &mut WorldPrimitives) {
    for (lo, hi, argb, width) in crate::game::remote::arena::world_boxes(game) {
        if width > 0.0 {
            primitives.box_outline(lo, hi, argb, width);
        } else {
            primitives.filled_box(lo, hi, argb);
        }
    }
}
