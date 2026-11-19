// The world's field of view divided by the zoom (CameraMixin.modifyFovWithZoom).
fn zoomify_fov(game: &crate::game::Game<'_>, partial: f32, camera: [f64; 3]) -> core::ops::ControlFlow<f32> {
    crate::game::fov::zoomify::fov(game, partial, camera)
}
