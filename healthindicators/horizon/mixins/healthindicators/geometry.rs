// Before the name tags: hearts, the number and armour points above entities.
fn healthindicators_geometry(
    game: &mut crate::game::Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<Vertex>,
    see_through: &mut Vec<Vertex>,
) {
    crate::game::remote::healthindicators::geometry(game, camera, alpha, out, see_through);
}
