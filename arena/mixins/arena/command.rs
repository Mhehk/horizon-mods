// `/arena …`, run on the client before the server sees it.
fn arena_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    use crate::game::remote::arena as m;
    let mut words = command.split_whitespace();
    if words.next() != Some("arena") {
        return;
    }
    *ran = true;
    if !game.native_on(m::ID) {
        game.native_chat(&format!("{}Mod kapalı (Options > Mods).", m::PREFIX));
        return;
    }
    let args: Vec<&str> = words.collect();
    let lines = m::command(game, &args);
    for line in lines {
        game.native_chat(&line);
    }
}
