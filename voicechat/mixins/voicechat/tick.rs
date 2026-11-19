// Each tick of the window's game (the `tick` hook point): on a server, where
// the player and the other players are, then the payloads the voice chat has
// for the server, its settings, keys bound on its screens and its chat lines.
fn voicechat_tick(game: &mut crate::game::Game<'_>) {
    use crate::game::remote::voicechat_world as world;
    use voicechat_engine as vc;
    // A single player world's server thread ticks a `Game` too: not ours.
    if !game.native_window() {
        return;
    }
    vc::opus::set_data_dir(game.native_data_dir(vc::ID));
    let address = match world::address(game) {
        Some(address) if game.native_on(vc::ID) => address,
        _ => {
            vc::tick(None, &|_| None);
            return;
        }
    };
    let players: Vec<(u128, [f64; 3])> = world::players(game)
        .into_iter()
        .map(|(uuid, [x, y, z])| (uuid, [x, y + vc::EYE_HEIGHT, z]))
        .collect();
    let listener = game.native_player().map(|p| vc::mix::Listener {
        eye: [p.x, p.y + vc::EYE_HEIGHT, p.z],
        yaw: p.yaw,
    });
    let name = |key: &str, default: &str| vc::key_label(&game.native_mod_key(vc::ID, key, default));
    let keys = vc::ui::KeyNames {
        menu: name(vc::KEY_MENU, vc::DEFAULT_MENU),
        ptt: name(vc::KEY_PTT, vc::DEFAULT_PTT),
        mute: name(vc::KEY_MUTE, vc::DEFAULT_MUTE),
    };
    let state = vc::World {
        address: &address,
        listener,
        players,
        redirect: horizon_protocol::server_redirect,
        skins: world::skins(game),
        keys,
    };
    let done = {
        let game = &*game;
        vc::tick(Some(state), &|key| game.native_setting(vc::ID, key))
    };
    for (channel, data) in done.payloads {
        let packet = horizon_protocol::packets::play::Serverbound::CustomPayload(
            horizon_protocol::packets::configuration::CustomPayload { channel, data },
        );
        if let Err(e) = game.native_send_packet(packet) {
            log::warn!("Horizon Voice Chat: a payload was not sent: {e}");
        }
    }
    for (key, value) in done.store {
        game.native_set_setting(vc::ID, &key, &value);
    }
    // A key chosen on the screens (push to talk), kept as Options > Mods keeps it.
    if !done.binds.is_empty() {
        for (name, key) in done.binds {
            game.settings.mods.keys.entry(vc::ID.to_owned()).or_default().insert(name, key);
        }
        game.settings.save(&crate::game::Game::options_path());
    }
    for line in done.chat {
        game.native_chat(&line);
    }
}
