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
        return;
    }
    if game.native_playing() {
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
        m::state().brush_held = false;
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
