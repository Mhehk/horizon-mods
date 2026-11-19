// The menu key (Open Hitreg Menu), Switch Hand and the score keys.
fn betterhitreg_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::remote::betterhitreg_config::key(game, key, pressed);
    crate::game::remote::betterhitreg_fight::key(game, key, pressed);
}
