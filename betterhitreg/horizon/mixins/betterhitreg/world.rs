// Once a frame while the world is drawn: the Render options (target hitbox, cross, ranges).
fn betterhitreg_world(game: &crate::game::Game<'_>, partial: f32, camera: [f64; 3], primitives: &mut WorldPrimitives) {
    crate::game::remote::betterhitreg_render::draw(game, partial, camera, primitives);
}
