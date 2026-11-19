// /healthindicators [offset|indicator-type|monitor|stop-monitoring].
fn healthindicators_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    if !*ran && crate::game::remote::healthindicators::command(game, command) {
        *ran = true;
    }
}
