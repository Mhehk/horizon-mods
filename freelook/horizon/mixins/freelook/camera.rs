// The camera takes the free look's rotation (CameraMixin.lockRotation).
fn freelook_camera(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<Option<CameraEntity>> {
    crate::game::perspective::freelook::camera_entity(game)
}
