// Each client tick: the metronome, the tutorial's lines, the settings file, then the fight.
fn betterhitreg_tick(game: &mut crate::game::Game<'_>) {
    crate::game::remote::betterhitreg_config::tick(game);
    crate::game::remote::betterhitreg_fight::tick(game);
    crate::game::remote::betterhitreg_fight::particle_flags(game);
}
