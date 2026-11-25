// Eksen — 16.10.2 (mihehk): the editor that drives the tools. It keeps the
// active tool and block, the history (Ctrl+Z / Ctrl+Y), the symmetry node,
// the builder tools' turn and flags and the capabilities, runs a tool on the
// stroke the player made, and turns what it answers into commands for the
// queue in `eksen` (state.rs). Behaviour after Axiom's documentation
// (.build-tools/axiom-docs/axiom-docs.md); no code, names or images of Axiom.
use std::collections::BTreeMap;

use super::eksen as st;
use super::eksen_abilities as ab;
use super::eksen_builder as b;
use super::eksen_core::{self as core, Blocks, Input, Outcome, Pos, Tool};

/// Vanilla's creative reach: past it Infinite Reach acts, vanilla does not.
pub const VANILLA_REACH: f64 = 5.0;

pub struct Editor {
    pub tools: Vec<Box<dyn Tool>>,
    /// The active tool (an index into `tools`).
    pub tool: usize,
    /// What tools place (the "Active Block").
    pub active: String,
    pub history: b::History,
    pub symmetry: b::Symmetry,
    pub abilities: ab::Abilities,
    pub flags: b::Flags,
    /// How the next move / clone is turned (Ctrl+R, Ctrl+F).
    pub transform: b::Transform,
    /// The tool's stroke while its key is held.
    pub stroke: Vec<Pos>,
    pub held: bool,
    pub secondary: bool,
    pub face: Pos,
    /// The attack button held (Bulldozer).
    pub breaking: bool,
    /// The text the Text and Annotation tools write (`/eksen metin …`).
    pub text: String,
}

impl Editor {
    pub fn new() -> Self {
        Editor {
            tools: core::all(),
            tool: 0,
            active: "minecraft:stone".into(),
            history: b::History::default(),
            symmetry: b::Symmetry::default(),
            abilities: ab::Abilities::new(),
            flags: b::Flags::default(),
            transform: b::Transform::default(),
            stroke: Vec::new(),
            held: false,
            secondary: false,
            face: [0, 1, 0],
            breaking: false,
            text: String::new(),
        }
    }

    pub fn tool_name(&self) -> &'static str {
        self.tools.get(self.tool).map_or("—", |t| t.name())
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

/// The face of block `hit` the eyes look at: a unit step out of the block,
/// along the axis the eyes are furthest from its centre.
pub fn face_towards(hit: Pos, eye: [f64; 3]) -> Pos {
    let d = [0, 1, 2].map(|i| eye[i] - (f64::from(hit[i]) + 0.5));
    let axis = (0..3).max_by(|a, b| d[*a].abs().total_cmp(&d[*b].abs())).unwrap_or(1);
    let mut f = [0; 3];
    f[axis] = if d[axis] >= 0.0 { 1 } else { -1 };
    f
}

/// `Entity.calculateViewVector`: 0 yaw faces south, positive pitch down.
pub fn view(yaw: f32, pitch: f32) -> [f64; 3] {
    let (y, p) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()]
}

/// The blocks of `edit` as commands: each row along x of one block becomes
/// one `fill` (a single block a `setblock`), with `tail` (` strict`).
pub fn edit_cmds(edit: &BTreeMap<Pos, String>, tail: &str) -> Vec<String> {
    let mut blocks: Vec<(&Pos, &String)> = edit.iter().collect();
    blocks.sort_by_key(|(p, _)| (p[1], p[2], p[0]));
    let mut runs: Vec<(Pos, Pos, &str)> = Vec::new();
    for (p, block) in blocks {
        match runs.last_mut() {
            Some((_, end, b)) if *b == block.as_str() && end[1] == p[1] && end[2] == p[2] && end[0] + 1 == p[0] => *end = *p,
            _ => runs.push((*p, *p, block.as_str())),
        }
    }
    runs.into_iter()
        .map(|(a, z, block)| {
            if a == z {
                format!("setblock {} {} {} {block}{tail}", a[0], a[1], a[2])
            } else {
                format!("fill {} {} {} {} {} {} {block}{tail}", a[0], a[1], a[2], z[0], z[1], z[2])
            }
        })
        .collect()
}

/// Sends `edit` the best way the server takes it: through its AxiomPaper
/// plugin as block buffers (every block string known to the game), else as
/// `fill` / `setblock` commands. Says how it went.
pub fn send(s: &mut st::State, world: &dyn Blocks, edit: &BTreeMap<Pos, String>) -> Result<String, String> {
    if s.axiom().takes_edits() {
        let ids = (|| {
            let dimension = world.dimension()?;
            let keep = world.state_id("minecraft:void_air")?;
            let ids: Option<BTreeMap<Pos, u32>> = edit.iter().map(|(p, b)| world.state_id(b).map(|i| (*p, i))).collect();
            Some((dimension, keep, ids?))
        })();
        if let Some((dimension, keep, ids)) = ids {
            let a = s.axiom();
            a.queue_edit(&dimension, &ids, keep);
            return Ok(format!("{} blok AxiomPaper ile gönderiliyor ({} bölüm sırada).", edit.len(), a.waiting()));
        }
    }
    let tail = s.editor().abilities.tail();
    let c = s.enqueue(edit_cmds(edit, tail))?;
    Ok(format!("{} blok, {c} komut sıraya alındı.", edit.len()))
}

/// Puts `edit` through the symmetry node, records it for undo (reading what
/// is there now) and sends it.
pub fn commit(s: &mut st::State, world: &dyn Blocks, edit: BTreeMap<Pos, String>, what: &str) -> Result<Vec<String>, String> {
    if edit.is_empty() {
        return Ok(vec![format!("{}{what}: değişecek blok yok.", st::PREFIX)]);
    }
    let edit = s.editor().symmetry.apply(edit);
    s.editor().history.record(world, &edit);
    let how = send(s, world, &edit)?;
    Ok(vec![format!("{}{what}: {how}", st::PREFIX)])
}

/// Runs the active tool on `input` and acts on what it answers.
pub fn run_tool(s: &mut st::State, world: &dyn Blocks, input: &Input) -> Vec<String> {
    let (outcome, name) = {
        let ed = s.editor();
        let Some(tool) = ed.tools.get(ed.tool) else {
            return vec![format!("{}Araç yok.", st::PREFIX)];
        };
        (tool.apply(world, input), tool.name())
    };
    let result = match outcome {
        Outcome::Edit(edit) => commit(s, world, edit, name),
        Outcome::Select(set) => {
            s.pos1 = None;
            s.pos2 = None;
            let n = set.len();
            let op = s.op;
            op.apply(&mut s.set, set);
            Ok(vec![format!("{}{name}: {n} blok · {} · seçim {} blok", st::PREFIX, op.name(), s.set.len())])
        }
        Outcome::Biomes(boxes) => {
            let cmds: Vec<String> = boxes
                .iter()
                .map(|(a, z, biome)| format!("fillbiome {} {} {} {} {} {} {biome}", a[0], a[1], a[2], z[0], z[1], z[2]))
                .collect();
            s.enqueue(cmds).map(|c| vec![format!("{}{name}: {c} biyom komutu sıraya alındı.", st::PREFIX)])
        }
        Outcome::Message(lines) => Ok(lines),
    };
    result.unwrap_or_else(|e| vec![format!("{}{e}", st::PREFIX)])
}

/// A stroke of the active tool begins at `hit` (the face looked at
/// `face`); `secondary` for its second action (N, the right button).
pub fn start_stroke(s: &mut st::State, hit: Option<Pos>, face: Pos, secondary: bool) {
    let e = s.editor();
    e.held = true;
    e.secondary = secondary;
    e.stroke = hit.into_iter().collect();
    e.face = face;
}

/// The stroke ends (the key or button let go): the tool runs on it.
pub fn finish_stroke(s: &mut st::State, world: &dyn Blocks, eye: [f64; 3], look: [f64; 3]) -> Vec<String> {
    let selection = s.set.clone();
    let e = s.editor();
    e.held = false;
    let input = Input {
        stroke: std::mem::take(&mut e.stroke),
        face: e.face,
        eye,
        look,
        active: e.active.clone(),
        selection,
        secondary: e.secondary,
        text: e.text.clone(),
    };
    if input.stroke.is_empty() {
        return vec![format!("{}Menzilde blok yok.", st::PREFIX)];
    }
    run_tool(s, world, &input)
}

/// The cuboid selection as the builder tools take it.
fn cuboid(s: &st::State) -> Result<b::Cuboid, String> {
    match (s.pos1, s.pos2) {
        (Some(a), Some(z)) if s.set.is_empty() => Ok(b::Cuboid::new(a, z)),
        _ => Err("Bu araç kutu seçimi ister: J ve K ile iki köşe seç.".into()),
    }
}

fn int(w: Option<&&str>, what: &str) -> Result<i32, String> {
    w.and_then(|w| w.parse().ok()).ok_or_else(|| format!("{what} bir tam sayı olmalı."))
}

/// The dominant axis of `look` (a unit step), as the builder tools nudge.
fn axis_of(look: [f64; 3]) -> Pos {
    let i = (0..3).max_by(|a, b| look[*a].abs().total_cmp(&look[*b].abs())).unwrap_or(0);
    let mut d = [0; 3];
    d[i] = if look[i] >= 0.0 { 1 } else { -1 };
    d
}

/// The editor's commands; None when `args[0]` is not one of them.
pub fn command(s: &mut st::State, args: &[&str], ctx: &st::Ctx<'_>) -> Option<Result<Vec<String>, String>> {
    let sub = *args.first()?;
    let p = st::PREFIX;
    let look = ctx.view.unwrap_or([0.0, 0.0, 1.0]);
    let world = ctx.world;
    Some((|| -> Result<Vec<String>, String> {
        match sub {
            "arac" | "araç" | "tool" => {
                let ed = s.editor();
                match args.get(1) {
                    None => {
                        let mut out = vec![format!("{p}Araçlar (/eksen arac <ad>), etkin: {}", ed.tool_name())];
                        out.push(ed.tools.iter().map(|t| format!("{} ({})", t.id(), t.name())).collect::<Vec<_>>().join(" · "));
                        Ok(out)
                    }
                    Some(id) => {
                        let i = ed.tools.iter().position(|t| t.id() == *id).ok_or_else(|| format!("Bilinmeyen araç: {id}"))?;
                        ed.tool = i;
                        let mut out = vec![format!("{p}Araç: {} (B basılı tut, N ikinci işlem)", ed.tools[i].name())];
                        out.extend(ed.tools[i].describe());
                        Ok(out)
                    }
                }
            }
            "ayar" | "option" => {
                let ed = s.editor();
                let tool = ed.tools.get_mut(ed.tool).ok_or("Araç yok.")?;
                match args.get(1) {
                    None => Ok(tool.describe()),
                    Some(opt) => {
                        tool.set(opt, args.get(2).copied())?;
                        Ok(tool.describe())
                    }
                }
            }
            "blok" | "block" => {
                let w = args.get(1).ok_or("Blok gerekli, ör. /eksen blok minecraft:oak_planks")?;
                if w.chars().any(|c| c.is_whitespace() || c == '/') {
                    return Err(format!("Geçersiz blok: {w}"));
                }
                let id = if w.contains(':') { (*w).to_owned() } else { format!("minecraft:{w}") };
                s.editor().active = id.clone();
                Ok(vec![format!("{p}Aktif blok: {id}")])
            }
            "metin" | "text" => {
                let text = args[1..].join(" ");
                if text.chars().count() > 256 {
                    return Err("Metin en çok 256 karakter olabilir.".into());
                }
                s.editor().text = text.clone();
                Ok(vec![format!("{p}Metin: \"{text}\" (Yazı ve Not araçları kullanır)")])
            }
            "geri" | "undo" => {
                let edit = s.editor().history.undo().ok_or("Geri alınacak değişiklik yok.")?;
                Ok(vec![format!("{p}Geri alındı: {}", send(s, world, &edit)?)])
            }
            "ileri" | "redo" => {
                let edit = s.editor().history.redo().ok_or("Yinelenecek değişiklik yok.")?;
                Ok(vec![format!("{p}Yinelendi: {}", send(s, world, &edit)?)])
            }
            "axiom" | "axiompaper" => {
                let a = s.axiom();
                match args.get(1).copied() {
                    Some("ac" | "aç" | "on") => a.disabled = false,
                    Some("kapat" | "off") => a.disabled = true,
                    Some("surum" | "sürüm" | "protocol") => {
                        a.protocol_override = int(args.get(2), "Protokol sürümü")?.max(0);
                    }
                    Some(w) => return Err(format!("Bilinmeyen: {w} (ac, kapat, surum <protokol>)")),
                    None => {}
                }
                Ok(a.lines())
            }
            "cevir" | "çevir" | "rotate" => {
                let ed = s.editor();
                ed.transform.rotate = (ed.transform.rotate + 1) % 4;
                Ok(vec![format!("{p}Döndürme: {}°", u32::from(ed.transform.rotate) * 90)])
            }
            "aynala" | "flip" => {
                let d = axis_of(look);
                let i = (0..3).find(|i| d[*i] != 0).unwrap_or(0);
                let ed = s.editor();
                ed.transform.flip[i] = !ed.transform.flip[i];
                Ok(vec![format!("{p}Aynalama: x {} · y {} · z {}", core::on_off(ed.transform.flip[0]), core::on_off(ed.transform.flip[1]), core::on_off(ed.transform.flip[2]))])
            }
            "bayrak" | "flag" => {
                let ed = s.editor();
                match args.get(1).copied() {
                    Some("koru" | "keep") => ed.flags.keep_existing = !ed.flags.keep_existing,
                    Some("hava" | "air") => ed.flags.copy_air = !ed.flags.copy_air,
                    _ => {}
                }
                Ok(vec![format!("{p}Var olanı koru {} · havayı kopyala {}", core::on_off(ed.flags.keep_existing), core::on_off(ed.flags.copy_air))])
            }
            "tasi" | "taşı" | "move" | "klon" | "clone" => {
                let src = cuboid(s)?;
                let d = [int(args.get(1), "x")?, int(args.get(2), "y")?, int(args.get(3), "z")?];
                let to = [0, 1, 2].map(|i| src.min[i] + d[i]);
                let (t, flags) = (s.editor().transform, s.editor().flags);
                let moving = matches!(sub, "tasi" | "taşı" | "move");
                let edit = if moving {
                    b::move_to(world, &src, t, to, flags)?
                } else {
                    let copies = args.get(4).map_or(Ok(1), |w| w.parse::<i32>().map_err(|_| "Adet bir sayı olmalı.".to_string()))?;
                    let targets: Vec<Pos> = (1..=copies.clamp(1, 64)).map(|k| [0, 1, 2].map(|i| src.min[i] + d[i] * k)).collect();
                    b::clone_to(world, &src, t, &targets, flags)?
                };
                if moving {
                    let size = b::turned_size(&src, t);
                    s.pos1 = Some(to);
                    s.pos2 = Some([0, 1, 2].map(|i| to[i] + size[i] - 1));
                }
                commit(s, world, edit, if moving { "Taşı" } else { "Klonla" })
            }
            "sur" | "sür" | "smear" => {
                let src = cuboid(s)?;
                let n = int(args.get(1), "Adet")?;
                let flags = s.editor().flags;
                let edit = b::smear(world, &src, axis_of(look), n, flags)?;
                commit(s, world, edit, "Sür")
            }
            "uzat" | "kisalt" | "kısalt" | "extrude" | "shrink" => {
                let hit = ctx.hit.ok_or("Bir bloğa bak.")?;
                let face = ctx.eye.map_or([0, 1, 0], |e| face_towards(hit, e));
                let shrink = matches!(sub, "kisalt" | "kısalt" | "shrink");
                commit(s, world, b::extrude(world, hit, face, shrink), if shrink { "Kısalt" } else { "Uzat" })
            }
            "silbagli" | "silbağlı" => {
                let hit = ctx.hit.ok_or("Bir bloğa bak.")?;
                commit(s, world, b::erase_connected(world, hit), "Bağlı sil")
            }
            "simetri" | "symmetry" => {
                let ed = s.editor();
                match args.get(1).copied() {
                    Some("nokta" | "node") => {
                        let hit = ctx.hit.ok_or("Bir bloğa bak.")?;
                        // On the looked-at face: twice the block's centre plus the face.
                        let face = ctx.eye.map_or([0, 1, 0], |e| face_towards(hit, e));
                        ed.symmetry.node2 = Some([0, 1, 2].map(|i| 2 * hit[i] + 1 + face[i]));
                    }
                    Some("merkez" | "centre") => {
                        let hit = ctx.hit.ok_or("Bir bloğa bak.")?;
                        ed.symmetry.node2 = Some(hit.map(|v| 2 * v + 1));
                    }
                    Some("aynala" | "flip") => {
                        let d = axis_of(look);
                        let i = (0..3).find(|i| d[*i] != 0).unwrap_or(0);
                        ed.symmetry.flip[i] = !ed.symmetry.flip[i];
                    }
                    Some("cevir" | "çevir" | "rotate") => ed.symmetry.cycle_rotate(),
                    Some("kaldir" | "kaldır" | "remove") => ed.symmetry = b::Symmetry::default(),
                    _ => {}
                }
                let sy = &ed.symmetry;
                let node = sy.node2.map_or("yok".to_owned(), |n| format!("{} {} {}", f64::from(n[0]) / 2.0, f64::from(n[1]) / 2.0, f64::from(n[2]) / 2.0));
                Ok(vec![format!(
                    "{p}Simetri {}: nokta {node} · aynala x {} y {} z {} · döndür {}",
                    if sy.on() { "açık" } else { "kapalı" },
                    core::on_off(sy.flip[0]),
                    core::on_off(sy.flip[1]),
                    core::on_off(sy.flip[2]),
                    sy.rotate
                )])
            }
            "yetenek" | "ability" => {
                let ed = s.editor();
                if let Some(w) = args.get(1) {
                    let a = ab::Ability::parse(w).ok_or_else(|| format!("Bilinmeyen yetenek: {w}"))?;
                    ed.abilities.toggle(a);
                }
                Ok(ed.abilities.lines())
            }
            "ucus" | "uçuş" | "kutu" | "toolbox" => {
                let ed = s.editor();
                if let Some(setting) = args.get(1) {
                    ed.abilities.set(setting, args.get(2).copied())?;
                }
                Ok(ed.abilities.lines())
            }
            _ => Err(String::new()),
        }
    })())
    .filter(|r| !matches!(r, Err(e) if e.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_become_rows() {
        let mut e = BTreeMap::new();
        for x in 0..4 {
            e.insert([x, 0, 0], "minecraft:stone".to_owned());
        }
        e.insert([5, 0, 0], "minecraft:stone".to_owned());
        e.insert([0, 1, 0], "minecraft:dirt".to_owned());
        let c = edit_cmds(&e, " strict");
        assert_eq!(
            c,
            vec![
                "fill 0 0 0 3 0 0 minecraft:stone strict".to_owned(),
                "setblock 5 0 0 minecraft:stone strict".to_owned(),
                "setblock 0 1 0 minecraft:dirt strict".to_owned(),
            ]
        );
    }

    #[test]
    fn faces_and_views() {
        assert_eq!(face_towards([0, 0, 0], [0.5, 3.0, 0.5]), [0, 1, 0]);
        assert_eq!(face_towards([0, 0, 0], [-4.0, 0.5, 0.5]), [-1, 0, 0]);
        let v = view(0.0, 0.0);
        assert!((v[2] - 1.0).abs() < 1e-9);
        assert_eq!(axis_of(view(90.0, 0.0)), [-1, 0, 0]);
    }
}
