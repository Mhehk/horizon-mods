// The menu key (V) opens the voice chat menu, and the settings, group and
// volumes keys their screens; push to talk and whisper while held; mute,
// voice chat off/on, hide icons and recording on a press. A key goes down
// only while playing, but letting go always counts.
fn voicechat_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    use voicechat_engine as vc;
    if !game.native_on(vc::ID) || key == vc::UNBOUND {
        return;
    }
    let menu = game.native_mod_key(vc::ID, vc::KEY_MENU, vc::DEFAULT_MENU);
    if pressed && key == menu && game.native_playing() {
        vc::menu_open(vc::key_char(key));
        if !game.native_mod_screen_open(vc::ID) {
            vc::menu_sync(false);
        }
        return;
    }
    let screens = [
        (vc::KEY_SETTINGS, vc::screens::Screen::Settings),
        (vc::KEY_GROUP, vc::screens::Screen::Group),
        (vc::KEY_VOLUMES, vc::screens::Screen::Volumes),
    ];
    for (name, screen) in screens {
        if pressed && game.native_playing() && game.native_mod_key(vc::ID, name, vc::UNBOUND) == key {
            vc::menu_open_at(screen, vc::key_char(key));
            if !game.native_mod_screen_open(vc::ID) {
                vc::menu_sync(false);
            }
            return;
        }
    }
    let bound = [
        (vc::Key::PushToTalk, vc::KEY_PTT, vc::DEFAULT_PTT),
        (vc::Key::Whisper, vc::KEY_WHISPER, vc::DEFAULT_WHISPER),
        (vc::Key::Mute, vc::KEY_MUTE, vc::DEFAULT_MUTE),
        (vc::Key::Disable, vc::KEY_DISABLE, vc::DEFAULT_DISABLE),
        (vc::Key::HideIcons, vc::KEY_HIDE_ICONS, vc::DEFAULT_HIDE_ICONS),
        (vc::Key::ToggleRecording, vc::KEY_RECORD, vc::UNBOUND),
    ];
    let Some(which) = bound.into_iter().find_map(|(which, name, default)| {
        let chosen = game.native_mod_key(vc::ID, name, default);
        (chosen == key && chosen != vc::UNBOUND).then_some(which)
    }) else {
        return;
    };
    if pressed && !game.native_playing() {
        return;
    }
    {
        let game = &*game;
        vc::load(&|k| game.native_setting(vc::ID, k));
    }
    if let Some(line) = vc::key(which, pressed) {
        game.native_chat(&line);
    }
    for (k, v) in vc::take_store() {
        game.native_set_setting(vc::ID, &k, &v);
    }
}
