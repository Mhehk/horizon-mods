// No first-person hand while the camera flies (GameRendererMixin.renderHand).
fn ozgurkamera_hand(canceled: &mut bool) {
    if crate::game::perspective::ozgurkamera::active() {
        *canceled = true;
    }
}
