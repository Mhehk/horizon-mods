// The hand's field of view with Affect Hand FOV (CameraMixin.modifyHudFovWithZoom).
fn zoomify_hud_fov(game: &crate::game::Game<'_>, partial: f32, camera: [f64; 3]) -> core::ops::ControlFlow<f32> {
    crate::game::fov::zoomify::hud_fov(game, partial, camera)
}
