// Relative view bobbing (CameraMixin.modifyBobbingIntensity).
fn zoomify_bob(game: &crate::game::Game<'_>, partial: f32) -> core::ops::ControlFlow<glam::Mat4> {
    crate::game::fov::zoomify::bob(game, partial)
}
