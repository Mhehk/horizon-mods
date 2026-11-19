// The editor's keys: corners at the looked-at block, clear, and the brush
// (held).
fn eksen_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    use crate::game::eksen as m;
    if !game.native_window() || !game.native_on(m::ID) {
        return;
    }
    let bound = |game: &crate::game::Game<'_>, i: usize| {
        game.native_mod_key(m::ID, m::KEYS[i].0, m::KEYS[i].2) == key
    };
    let (pos1, pos2, clear, brush) = (bound(game, 0), bound(game, 1), bound(game, 2), bound(game, 3));
    if brush {
        let mut s = m::state();
        s.brush_held = pressed && game.native_playing() && s.brush.is_some();
        s.last_brush = None;
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
            if pos1 {
                s.pos1 = Some(p);
            } else {
                s.pos2 = Some(p);
            }
            s.describe(if pos1 { 1 } else { 2 }, p)
        };
        game.native_chat(&line);
    } else if clear {
        {
            let mut s = m::state();
            s.pos1 = None;
            s.pos2 = None;
        }
        game.native_chat(&format!("{}Seçim temizlendi.", m::PREFIX));
    }
}
