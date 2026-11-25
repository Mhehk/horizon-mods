// The editor's keys: corners at the looked-at block, clear, the brush (held),
// Magic Select (held), the active tool and its second action (held: the
// stroke; released: the tool runs), Replace Mode (R), and Axiom's shortcuts
// Ctrl+Z / Ctrl+Y (undo / redo), Ctrl+R (rotate), Ctrl+F (flip). The mouse
// buttons drive the capabilities that work through commands.
fn eksen_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    use crate::game::eksen as m;
    use crate::game::eksen_abilities::Ability;
    use crate::game::eksen_editor as ed;
    if !game.native_window() || !game.native_on(m::ID) {
        return;
    }
    let bound = |game: &crate::game::Game<'_>, i: usize| {
        game.native_mod_key(m::ID, m::KEYS[i].0, m::KEYS[i].2) == key
    };
    let [pos1, pos2, clear, brush, magic, tool, tool2, replace, editor] = [0, 1, 2, 3, 4, 5, 6, 7, 8].map(|i| bound(game, i));
    let player = game.native_player();
    let eye = player.as_ref().map(|p| [p.x, p.y + 1.62, p.z]);
    let look = player.as_ref().map(|p| ed::view(p.yaw, p.pitch));

    // Right Shift: the editor screen (closing it is the screen's own key
    // hook, ui_keys.rs).
    if editor && pressed && game.native_playing() {
        if game.native_mod_screen_open(m::ID) {
            m::state().ui().open = true;
        }
        return;
    }

    // Ctrl shortcuts, as Axiom's.
    if pressed && game.native_playing() && game.modifiers.control_key() {
        let sub = match key {
            "key.keyboard.z" => Some("geri"),
            "key.keyboard.y" => Some("ileri"),
            "key.keyboard.r" => Some("cevir"),
            "key.keyboard.f" => Some("aynala"),
            _ => None,
        };
        if let Some(sub) = sub {
            let lines = {
                let world = m::GameBlocks(&*game);
                let ctx = m::Ctx { feet: None, look: None, hit: None, eye, view: look, world: &world };
                m::run(&[sub], &ctx)
            };
            for line in lines {
                game.native_chat(&line);
            }
            return;
        }
    }

    // UNVERIFIED: that Horizon passes the mouse buttons to this hook as
    // `key.mouse.left` / `key.mouse.right` (as the key names it stores).
    if key == "key.mouse.left" {
        m::state().editor().breaking = pressed;
    }
    if key == "key.mouse.right" && pressed && game.native_playing() {
        let (reach, infinite, angel, active, tail) = {
            let mut s = m::state();
            let e = s.editor();
            (
                e.abilities.reach(),
                e.abilities.is_on(Ability::InfiniteReach),
                e.abilities.is_on(Ability::AngelPlacement),
                e.active.clone(),
                e.abilities.tail(),
            )
        };
        let hit = game.native_raycast(reach).map(|h| [h.block_x, h.block_y, h.block_z]);
        let target = match (hit, eye, look) {
            // Infinite Reach: past vanilla's reach, at the face looked at.
            (Some(h), Some(e), _) if infinite => {
                let d = [0, 1, 2].map(|i| f64::from(h[i]) + 0.5 - e[i]);
                let far = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() > ed::VANILLA_REACH;
                far.then(|| {
                    let f = ed::face_towards(h, e);
                    [h[0] + f[0], h[1] + f[1], h[2] + f[2]]
                })
            }
            // Angel Placement: nothing in reach, so in the air ahead.
            (None, Some(e), Some(l)) if angel => Some(crate::game::eksen_abilities::angel_target(e, l, 4.0)),
            _ => None,
        };
        if let Some(t) = target {
            // Through AxiomPaper when the server has it (no operator rights
            // needed), else as a command.
            let id = {
                use crate::game::eksen_magic::Blocks as _;
                m::GameBlocks(&*game).state_id(&active)
            };
            let mut s = m::state();
            let updates = tail.is_empty();
            let placed = id.is_some_and(|id| s.axiom().place(t, id, 1, angel && hit.is_none(), updates));
            if !placed {
                let _ = s.enqueue(vec![format!("setblock {} {} {} {active}{tail}", t[0], t[1], t[2])]);
            }
        }
    }

    if replace && pressed && game.native_playing() {
        let on = m::state().editor().abilities.toggle(Ability::ReplaceMode);
        game.native_chat(&format!("{}Değiştirme modu {} (Horizon bağlantısı bekliyor).", m::PREFIX, if on { "açık" } else { "kapalı" }));
        return;
    }
    if brush {
        let mut s = m::state();
        s.brush_held = pressed && game.native_playing() && s.brush.is_some();
        s.last_brush = None;
        return;
    }
    if magic {
        if pressed && game.native_playing() {
            let hit = game.native_raycast(m::REACH).map(|h| [h.block_x, h.block_y, h.block_z]);
            let mut s = m::state();
            s.magic_held = true;
            s.magic_seeds = hit.into_iter().collect();
        } else if !pressed && m::state().magic_held {
            let line = m::state().finish_magic(&m::GameBlocks(game));
            game.native_chat(&line);
        }
        return;
    }
    if tool || tool2 {
        if pressed && game.native_playing() {
            let hit = game.native_raycast(m::REACH).map(|h| [h.block_x, h.block_y, h.block_z]);
            let face = match (hit, eye) {
                (Some(h), Some(ey)) => ed::face_towards(h, ey),
                _ => [0, 1, 0],
            };
            ed::start_stroke(&mut m::state(), hit, face, tool2);
        } else if !pressed && m::state().editor().held {
            let lines = {
                let world = m::GameBlocks(&*game);
                let mut s = m::state();
                ed::finish_stroke(&mut s, &world, eye.unwrap_or_default(), look.unwrap_or([0.0, 0.0, 1.0]))
            };
            for line in lines {
                game.native_chat(&line);
            }
        }
        return;
    }
    if !pressed || !game.native_playing() {
        return;
    }
    if pos1 || pos2 {
        let Some(hit) = game.native_raycast(m::REACH) else {
            game.native_chat(&format!("{}Menzilde blok yok.", m::PREFIX));
            return;
        };
        let p = [hit.block_x, hit.block_y, hit.block_z];
        let line = {
            let mut s = m::state();
            let which = if pos1 { 1 } else { 2 };
            s.set_corner(which, p);
            s.describe(which, p)
        };
        game.native_chat(&line);
    } else if clear {
        m::state().clear_selection();
        game.native_chat(&format!("{}Seçim temizlendi.", m::PREFIX));
    }
}
