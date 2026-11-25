// Each window tick: paint with the brush while its key is held, then send a
// few queued commands to the server.
fn eksen_tick(game: &mut crate::game::Game<'_>) {
    use crate::game::eksen as m;
    if !game.native_window() {
        return;
    }
    if !game.native_on(m::ID) {
        let mut s = m::state();
        s.queue.clear();
        s.job = 0;
        s.axiom().out.clear();
        return;
    }
    // AxiomPaper: what the link has for the server (the hello answer, the
    // fly speed, the block buffers its rate limit lets through this tick).
    let (payloads, notes) = {
        let mut s = m::state();
        let fly = s.editor().abilities.flying_speed();
        let a = s.axiom();
        if game.native_player().is_none() {
            a.reset();
        }
        a.fly_speed(fly);
        a.tick();
        (std::mem::take(&mut a.out), std::mem::take(&mut a.notes))
    };
    for (channel, data) in payloads {
        let packet = horizon_protocol::packets::play::Serverbound::CustomPayload(
            horizon_protocol::packets::configuration::CustomPayload { channel, data },
        );
        if let Err(e) = game.native_send_packet(packet) {
            log::warn!("Eksen: an AxiomPaper payload was not sent: {e}");
        }
    }
    for note in notes {
        game.native_chat(&format!("{}{note}", m::PREFIX));
    }
    if game.native_playing() {
        // A tool held: each new block looked at joins its stroke.
        let (tool_held, bulldozer, reach, tail) = {
            let mut s = m::state();
            let e = s.editor();
            let a = &e.abilities;
            (
                e.held,
                e.breaking && a.is_on(crate::game::eksen_abilities::Ability::Bulldozer),
                a.reach(),
                a.tail(),
            )
        };
        // (On the editor screen the pointer draws the stroke, ui_drag.rs.)
        if tool_held && game.native_mod_screen() != Some(m::ID) {
            if let Some(hit) = game.native_raycast(m::REACH) {
                let p = [hit.block_x, hit.block_y, hit.block_z];
                let mut s = m::state();
                let e = s.editor();
                if e.stroke.len() < 4096 && e.stroke.last() != Some(&p) && !e.stroke.contains(&p) {
                    e.stroke.push(p);
                }
            }
        }
        // Bulldozer: while attack is held, the looked-at block breaks every
        // tick instead of vanilla's creative break delay.
        if bulldozer {
            if let Some(hit) = game.native_raycast(reach) {
                let mut s = m::state();
                if s.queue.len() < 64 {
                    let _ = s.enqueue(vec![format!("setblock {} {} {} minecraft:air{tail}", hit.block_x, hit.block_y, hit.block_z)]);
                }
            }
        }
        // Magic Select held: each new block looked at joins the kinds it takes.
        if m::state().magic_held {
            if let Some(hit) = game.native_raycast(m::REACH) {
                let p = [hit.block_x, hit.block_y, hit.block_z];
                let mut s = m::state();
                if s.magic_seeds.len() < 256 && !s.magic_seeds.contains(&p) {
                    s.magic_seeds.push(p);
                }
            }
        }
        let brush = {
            let s = m::state();
            if s.brush_held && s.queue.len() < 256 {
                s.brush.clone()
            } else {
                None
            }
        };
        if let Some(brush) = brush {
            if let Some(hit) = game.native_raycast(m::REACH) {
                let c = [hit.block_x, hit.block_y, hit.block_z];
                let mut s = m::state();
                if s.last_brush != Some(c) {
                    s.last_brush = Some(c);
                    let st = &mut *s;
                    let before = st.queue.len();
                    m::sphere(&brush, c, &mut st.queue);
                    st.job += st.queue.len() - before;
                }
            }
        }
    } else {
        let mut s = m::state();
        s.brush_held = false;
        s.magic_held = false;
        s.magic_seeds.clear();
    }
    for _ in 0..m::PER_TICK {
        let next = m::state().queue.pop_front();
        let Some(cmd) = next else {
            break;
        };
        if let Err(e) = game.native_send_chat(&format!("/{cmd}")) {
            {
                let mut s = m::state();
                s.queue.clear();
                s.job = 0;
            }
            game.native_chat(&format!("{}Komut gönderilemedi, kuyruk durdu: {e}", m::PREFIX));
            return;
        }
    }
    let done = {
        let mut s = m::state();
        if s.queue.is_empty() && s.job > 0 && !s.brush_held {
            let n = s.job;
            s.job = 0;
            Some(n)
        } else {
            None
        }
    };
    if let Some(n) = done {
        game.native_chat(&format!("{}Tamamlandı ({n} komut).", m::PREFIX));
    }
}
