// `/eksen ...`, run on the client before the server sees it.
fn eksen_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    use crate::game::eksen as m;
    let mut words = command.split_whitespace();
    if words.next() != Some("eksen") {
        return;
    }
    *ran = true;
    if !game.native_on(m::ID) {
        game.native_chat(&format!("{}Mod kapalı (Options > Mods).", m::PREFIX));
        return;
    }
    let args: Vec<&str> = words.collect();
    let player = game.native_player();
    let ctx = m::Ctx {
        feet: player
            .as_ref()
            .map(|p| [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32]),
        look: player.as_ref().map(|p| m::look_dir(p.yaw, p.pitch)),
        hit: game
            .native_raycast(m::REACH)
            .map(|h| [h.block_x, h.block_y, h.block_z]),
    };
    for line in m::run(&args, &ctx) {
        game.native_chat(&line);
    }
}
