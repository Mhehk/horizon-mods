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
    let lines = {
        let world = m::GameBlocks(&*game);
        let ctx = m::Ctx {
            feet: player
                .as_ref()
                .map(|p| [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32]),
            look: player.as_ref().map(|p| m::look_dir(p.yaw, p.pitch)),
            hit: game
                .native_raycast(m::REACH)
                .map(|h| [h.block_x, h.block_y, h.block_z]),
            eye: player.as_ref().map(|p| [p.x, p.y + 1.62, p.z]),
            view: player.as_ref().map(|p| crate::game::eksen_editor::view(p.yaw, p.pitch)),
            world: &world,
        };
        m::run(&args, &ctx)
    };
    for line in lines {
        game.native_chat(&line);
    }
}
