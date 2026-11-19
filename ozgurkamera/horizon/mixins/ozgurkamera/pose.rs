// The world is drawn from the free camera.
fn ozgurkamera_pose(position: &mut [f64; 3], yaw: &mut f32, pitch: &mut f32) {
    crate::game::perspective::ozgurkamera::pose(position, yaw, pitch);
}
