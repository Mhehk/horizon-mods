//! Simple Voice Chat's own pictures (its screens' backgrounds and its icons),
//! read at run time from the player's own copy of the Simple Voice Chat jar,
//! never shipped with this mod: Simple Voice Chat is "All Rights Reserved", so
//! its textures may not be redistributed, but a player may look at the copy
//! they already have. Found in, in order: the mod's data folder (any
//! `voicechat*.jar`, or a `svc/` folder with `assets/voicechat/...` laid out),
//! then the launchers' Fabric/Quilt/NeoForge mod folders. Without one, the
//! screens use the mod's own look (`skin`).
use crate::ui::Draw;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

/// A texture's pixels, ARGB, row by row.
pub struct Pixels {
    pub w: usize,
    pub h: usize,
    argb: Vec<u32>,
}

enum Source {
    Jar(PathBuf),
    Folder(PathBuf),
}

struct Store {
    source: Option<Source>,
    searched: bool,
    textures: HashMap<String, Option<Pixels>>,
}

static STORE: LazyLock<Mutex<Store>> =
    LazyLock::new(|| Mutex::new(Store { source: None, searched: false, textures: HashMap::new() }));

/// A Simple Voice Chat version from a jar's name (`voicechat-fabric-1.21.11-2.6.22.jar` → 2.6.22), to pick the newest.
fn version_of(name: &str) -> Vec<u32> {
    name.rsplit('-').next().unwrap_or("").trim_end_matches(".jar").split(['.', '+']).filter_map(|p| p.parse().ok()).collect()
}

fn jars_in(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_lowercase();
        if name.starts_with("voicechat") && name.ends_with(".jar") && !name.contains("api") {
            out.push(e.path());
        }
    }
}

/// Where launchers keep mods: Dawn/Feather, Modrinth, Prism/MultiMC, CurseForge and the vanilla folder.
fn launcher_jars() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) else { return out };
    let profile_dirs = [
        (appdata.join(".dawn").join("profiles"), "private-game-content/mods"),
        (appdata.join("ModrinthApp").join("profiles"), "mods"),
        (appdata.join("PrismLauncher").join("instances"), ".minecraft/mods"),
        (appdata.join("com.modrinth.theseus").join("profiles"), "mods"),
    ];
    for (root, sub) in profile_dirs {
        if let Ok(entries) = std::fs::read_dir(&root) {
            for e in entries.flatten() {
                jars_in(&e.path().join(sub), &mut out);
            }
        }
    }
    jars_in(&appdata.join(".minecraft").join("mods"), &mut out);
    if let Some(home) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
        if let Ok(entries) = std::fs::read_dir(home.join("curseforge").join("minecraft").join("Instances")) {
            for e in entries.flatten() {
                jars_in(&e.path().join("mods"), &mut out);
            }
        }
    }
    out
}

fn find() -> Option<Source> {
    if let Some(dir) = crate::dylib::data_dir() {
        let folder = dir.join("svc");
        if folder.join("assets").join("voicechat").is_dir() {
            return Some(Source::Folder(folder));
        }
        let mut own = Vec::new();
        jars_in(dir, &mut own);
        if let Some(j) = own.into_iter().max_by_key(|p| version_of(&p.to_string_lossy())) {
            return Some(Source::Jar(j));
        }
    }
    let jars = launcher_jars();
    jars.into_iter().max_by_key(|p| version_of(&p.file_name().unwrap_or_default().to_string_lossy())).map(Source::Jar)
}

fn read(source: &Source, path: &str) -> Result<Vec<u8>, String> {
    match source {
        Source::Folder(dir) => std::fs::read(dir.join(path)).map_err(|e| e.to_string()),
        Source::Jar(jar) => {
            let file = std::fs::File::open(jar).map_err(|e| e.to_string())?;
            let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file)).map_err(|e| e.to_string())?;
            let mut entry = zip.by_name(path).map_err(|e| e.to_string())?;
            let mut out = Vec::new();
            entry.read_to_end(&mut out).map_err(|e| e.to_string())?;
            Ok(out)
        }
    }
}

fn decode(bytes: &[u8]) -> Result<Pixels, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let (w, h) = (info.width as usize, info.height as usize);
    let channels = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Grayscale => 1,
        png::ColorType::Indexed => return Err("indexed colours left after expanding".into()),
    };
    let line = info.line_size;
    let mut argb = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let p = &buf[y * line + x * channels..][..channels];
            let (r, g, b, a) = match channels {
                4 => (p[0], p[1], p[2], p[3]),
                3 => (p[0], p[1], p[2], 255),
                2 => (p[0], p[0], p[0], p[1]),
                _ => (p[0], p[0], p[0], 255),
            };
            argb.push(u32::from(a) << 24 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b));
        }
    }
    Ok(Pixels { w, h, argb })
}

/// Whether the player's Simple Voice Chat jar was found (and so its look is drawn).
pub fn available() -> bool {
    let mut s = STORE.lock().unwrap_or_else(|e| e.into_inner());
    if !s.searched {
        s.searched = true;
        s.source = find();
        match &s.source {
            Some(Source::Jar(p)) => log::info!(target: "horizon_voicechat", "Voice Chat: Simple Voice Chat's look from {}", p.display()),
            Some(Source::Folder(p)) => log::info!(target: "horizon_voicechat", "Voice Chat: Simple Voice Chat's look from {}", p.display()),
            None => log::info!(target: "horizon_voicechat", "Voice Chat: no Simple Voice Chat jar found, using the mod's own look"),
        }
    }
    s.source.is_some()
}

/// `texture` (`gui/gui_voicechat.png`, `icons/microphone.png`) loaded once.
fn with<R>(texture: &str, f: impl FnOnce(&Pixels) -> R) -> Option<R> {
    if !available() {
        return None;
    }
    let mut s = STORE.lock().unwrap_or_else(|e| e.into_inner());
    if !s.textures.contains_key(texture) {
        let path = format!("assets/voicechat/textures/{texture}");
        let loaded = s.source.as_ref().map(|src| read(src, &path).and_then(|b| decode(&b)));
        let pixels = match loaded {
            Some(Ok(p)) => Some(p),
            Some(Err(e)) => {
                log::warn!(target: "horizon_voicechat", "Voice Chat: {path} not read: {e}");
                None
            }
            None => None,
        };
        s.textures.insert(texture.to_owned(), pixels);
    }
    s.textures.get(texture)?.as_ref().map(f)
}

/// The size of `texture` in pixels.
pub fn size(texture: &str) -> Option<[usize; 2]> {
    with(texture, |p| [p.w, p.h])
}

/// Part `src` (pixels) of `texture` drawn into `dst` (GUI pixels), as fills
/// of the runs of one colour along each row. None without the texture.
pub fn fills(texture: &str, src: [f32; 4], dst: [f32; 4]) -> Option<Vec<Draw>> {
    with(texture, |p| pixel_fills(p, src, dst))
}

fn pixel_fills(p: &Pixels, src: [f32; 4], dst: [f32; 4]) -> Vec<Draw> {
    let (sx, sy) = (dst[2] / src[2].max(1.0), dst[3] / src[3].max(1.0));
    let x0 = src[0].max(0.0) as usize;
    let y0 = src[1].max(0.0) as usize;
    let x1 = ((src[0] + src[2]) as usize).min(p.w);
    let y1 = ((src[1] + src[3]) as usize).min(p.h);
    let mut out = Vec::new();
    for y in y0..y1 {
        let row = &p.argb[y * p.w..][..p.w];
        let mut x = x0;
        while x < x1 {
            let c = row[x];
            let mut end = x + 1;
            while end < x1 && row[end] == c {
                end += 1;
            }
            if c >> 24 != 0 {
                out.push(Draw::Fill([dst[0] + (x - x0) as f32 * sx, dst[1] + (y - y0) as f32 * sy, (end - x) as f32 * sx, sy], c));
            }
            x = end;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_version_wins() {
        assert!(version_of("voicechat-fabric-1.21.11-2.6.22.jar") > version_of("voicechat-fabric-1.21.11-2.6.9.jar"));
        assert_eq!(version_of("voicechat-fabric-1.21.11-2.6.22.jar"), vec![2, 6, 22]);
    }

    /// With a Simple Voice Chat jar on this machine, its main screen is read.
    #[test]
    fn reads_the_players_own_jar_when_there_is_one() {
        if !available() {
            return;
        }
        assert_eq!(size("gui/gui_voicechat.png"), Some([256, 256]));
        assert_eq!(size("icons/microphone_button.png"), Some([32, 32]));
        assert!(fills("icons/microphone.png", [0.0, 0.0, 16.0, 16.0], [0.0, 0.0, 16.0, 16.0]).is_some_and(|f| !f.is_empty()));
    }

    #[test]
    fn runs_of_one_colour_are_one_fill() {
        let p = Pixels { w: 4, h: 1, argb: vec![0xFF00_0000, 0xFF00_0000, 0, 0xFFFF_FFFF] };
        let f = pixel_fills(&p, [0.0, 0.0, 4.0, 1.0], [10.0, 10.0, 8.0, 2.0]);
        assert_eq!(f.len(), 2);
        assert_eq!(f[0], Draw::Fill([10.0, 10.0, 4.0, 2.0], 0xFF00_0000));
    }
}
