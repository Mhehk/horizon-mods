// The TAB list's players, as `remote_tab_players` builds them, with two local
// drawing changes: heads on every server (vanilla hides them on offline mode
// servers; only the list's drawing changes, never the connection or chat), and
// the hat layer only for players inside the active chunk view whose chunk is
// loaded. Turned off, the original body runs.
fn hatlayers_tab(
    game: &mut crate::game::Game<'_>,
) -> std::ops::ControlFlow<Option<(Vec<horizon_client::tab_list::TabPlayer>, bool, Vec<u128>)>> {
    if !game.native_on("hatlayers") {
        horizon_client::tab_list::hatlayers_marker::set_active(false);
        return std::ops::ControlFlow::Continue(());
    }
    // The original body, up to its result.
    game.update_remote_skins();
    game.publish_tab_object_skins();
    let game = &*game;
    let result = (|| {
        let mut players: Vec<_> = {
            let r = game.remote.as_ref()?;
            r.listed
                .iter()
                .filter_map(|u| r.tab_players.get(u).cloned())
                .collect()
        };
        for p in &mut players {
            p.skin = game
                .remote
                .as_ref()
                .and_then(|r| r.skins.get(&p.uuid).cloned())
                .unwrap_or_else(|| horizon_client::tab_list::default_skin(p.uuid).to_owned());
        }
        let r = game.remote.as_ref()?;
        let upside_down: Vec<u128> = players
            .iter()
            .filter(|p| {
                (p.name == "Dinnerbone" || p.name == "Grumm")
                    && (p.uuid == r.profile || r.level_players.values().any(|u| *u == p.uuid))
            })
            .map(|p| p.uuid)
            .collect();
        // Hat Layers: the hat only inside the active chunk view (HatVisibility),
        // and each decision kept for `/hatlayers` (ChunkHatDiagnostics.recordDraw).
        for p in players.iter_mut() {
            p.show_hat = crate::game::remote::hatlayers::inspect(game, p.uuid).show_hat;
            crate::game::remote::hatlayers::record(p.uuid, p.show_hat);
        }
        // Heads on every server: the list's own `online_mode` only.
        Some((players, true, upside_down))
    })();
    // The marker goes on hat heads only for a list HatLayers built.
    horizon_client::tab_list::hatlayers_marker::set_active(result.is_some());
    std::ops::ControlFlow::Break(result)
}
