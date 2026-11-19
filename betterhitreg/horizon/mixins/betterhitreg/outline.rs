// GlowMixin: the target glows in the perfect hit or jump reset colour.
fn betterhitreg_outline(game: &crate::game::Game<'_>, entity_id: u32, outline: &mut Option<u32>) {
    crate::game::remote::betterhitreg_fight::outline(game, entity_id, outline);
}
