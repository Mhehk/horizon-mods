// The player gets neutral input while the camera flies (BLOCKED_INPUT).
fn ozgurkamera_movement(forward: &mut f32, strafe: &mut f32, jump: &mut bool, sneak: &mut bool) {
    if crate::game::perspective::ozgurkamera::active() {
        *forward = 0.0;
        *strafe = 0.0;
        *jump = false;
        *sneak = false;
    }
}
