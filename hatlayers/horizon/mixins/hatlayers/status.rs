// Hat Layers — mhv1.2 (mihehk): which TAB heads show their hat layer
// (HatVisibility, ActiveChunkRange) and the local `/hatlayers` report
// (ChunkHatDiagnostics). All output is local chat; nothing is sent to the server.
use std::cell::RefCell;
use std::time::Instant;

pub(crate) const VERSION: &str = "mhv1.2";

pub(crate) struct Status {
    pub(crate) show_hat: bool,
    pub(crate) reason: &'static str,
    pub(crate) view: i64,
    pub(crate) distance: i64,
}

fn chunk_of(x: f64, z: f64) -> (i64, i64) {
    ((x.floor() as i64).div_euclid(16), (z.floor() as i64).div_euclid(16))
}

/// `ActiveChunkRange.contains`: horizontal only; height and line of sight do not matter.
fn contains(center: (i64, i64), target: (i64, i64), view: i64) -> bool {
    view >= 0 && (target.0 - center.0).abs() <= view && (target.1 - center.1).abs() <= view
}

fn status(show_hat: bool, reason: &'static str, view: i64, distance: i64) -> Status {
    Status { show_hat, reason, view, distance }
}

/// `HatVisibility.inspect`.
pub(crate) fn inspect(game: &crate::game::Game<'_>, uuid: u128) -> Status {
    let (Some(r), Some(me)) = (game.remote.as_ref(), game.native_player()) else {
        return status(false, "Dünya hazır değil", 0, -1);
    };
    let own = game.settings.render_distance as i64;
    let view = if r.view_distance > 0 { own.min(r.view_distance as i64) } else { own };
    let center = chunk_of(me.x, me.z);
    let target = if uuid == r.profile {
        Some(center)
    } else {
        r.level_players
            .iter()
            .find(|(_, u)| **u == uuid)
            .and_then(|(id, _)| game.mobs.iter().find(|m| m.id as i32 == *id && m.alive()))
            .map(|m| chunk_of(m.position.x, m.position.z))
    };
    let Some(target) = target else {
        return status(false, "Sunucu bu oyuncunun konumunu göndermiyor", view, -1);
    };
    let distance = (target.0 - center.0).abs().max((target.1 - center.1).abs());
    if !contains(center, target, view) {
        return status(false, "Aktif mesafenin dışında", view, distance);
    }
    if !game.world.chunks.contains_key(&(target.0 as i32, target.1 as i32)) {
        return status(false, "Oyuncunun chunk'ı yüklü değil", view, distance);
    }
    status(true, "Aktif chunk içinde", view, distance)
}

thread_local! {
    /// The last TAB draw of each player, oldest first, at most 160.
    static LAST_DRAWS: RefCell<Vec<(u128, bool, Instant)>> = const { RefCell::new(Vec::new()) };
}

/// `ChunkHatDiagnostics.recordDraw`.
pub(crate) fn record(uuid: u128, show_hat: bool) {
    LAST_DRAWS.with(|d| {
        let mut d = d.borrow_mut();
        if let Some(e) = d.iter_mut().find(|e| e.0 == uuid) {
            *e = (uuid, show_hat, Instant::now());
            return;
        }
        // Bounded even on servers with rotating fake TAB entries.
        if d.len() >= 160 {
            d.remove(0);
        }
        d.push((uuid, show_hat, Instant::now()));
    });
}

fn say(game: &mut crate::game::Game<'_>, message: &str) {
    game.native_chat(&format!("[Hat Layers] {message}"));
}

fn on_off(on: bool) -> &'static str {
    if on { "AÇIK" } else { "KAPALI" }
}

/// `ChunkHatDiagnostics.handle`: `/hatlayers [name]` or `/chunkhat [name]`.
pub(crate) fn handle(game: &mut crate::game::Game<'_>, command: &str) -> bool {
    let root = command.split(' ').next().unwrap_or("");
    if root != "hatlayers" && root != "chunkhat" {
        return false;
    }
    let Some(r) = game.remote.as_ref() else {
        return true;
    };
    let entries: Vec<(u128, String)> = r
        .tab_players
        .iter()
        .map(|(u, p)| (*u, p.name.clone()))
        .collect();
    let name = command[root.len()..].trim().to_owned();
    say(game, &format!("Hat Layers {VERSION} | Yüz kalır; yalnızca ikinci kafa katmanı kontrol edilir."));
    let mut displayed = 0;
    for (uuid, player) in entries {
        if !name.is_empty() && player.to_lowercase() != name.to_lowercase() {
            continue;
        }
        let s = inspect(game, uuid);
        // Without a name, tracked players only, not a large server's whole TAB list.
        if name.is_empty() && s.distance < 0 {
            continue;
        }
        let distance = if s.distance < 0 {
            "bilinmiyor".to_owned()
        } else {
            format!("{} chunk", s.distance)
        };
        say(
            game,
            &format!(
                "{player}: hat {} | {} | uzaklık {distance} | sınır {}",
                on_off(s.show_hat),
                s.reason,
                s.view
            ),
        );
        let drawn = LAST_DRAWS.with(|d| d.borrow().iter().find(|e| e.0 == uuid).map(|e| (e.1, e.2)));
        match drawn {
            Some((show, at)) => say(
                game,
                &format!("Son TAB çizimi: hat {} ({} sn önce)", on_off(show), at.elapsed().as_secs()),
            ),
            None => say(game, "Bu oyuncu için henüz TAB çizimi kaydedilmedi. TAB'ı açıp tekrar kontrol et."),
        }
        displayed += 1;
        if displayed >= 10 {
            break;
        }
    }
    if displayed == 0 && !name.is_empty() {
        say(game, &format!("TAB listesinde bu isim bulunamadı: {name}"));
    }
    if name.is_empty() {
        say(game, "Belirli bir oyuncu için: /hatlayers OyuncuAdı");
    }
    true
}
