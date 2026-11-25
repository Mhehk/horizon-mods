// Arena HUD — 16.10.1 (mihehk): the HUD editor (Right Alt). Every HUD
// module shows in a box; drag one to move it, and it snaps to the screen's
// edges and centre and to the other modules' edges; a right click puts it
// back where it started. The list on the left turns modules on and off.
// Plain Rust: the hooks draw what this lays out and store what it moves.
use super::arena_core::{Draw, Module, ALL};

/// How close (GUI pixels) an edge has to come to snap.
pub const SNAP: f32 = 4.0;
const LIST_W: f32 = 118.0;
const ROW: f32 = 11.0;

/// A module's place on screen as stored: fractions of the free space
/// (`x = fx * (width - w)`), so a module keeps its side when the window
/// changes size.
pub fn at(fraction: [f32; 2], size: [f32; 2], screen: [f32; 2]) -> [f32; 2] {
    [0, 1].map(|i| (fraction[i].clamp(0.0, 1.0) * (screen[i] - size[i]).max(0.0)).round())
}

/// The stored fractions of a module at `pos`.
pub fn fraction(pos: [f32; 2], size: [f32; 2], screen: [f32; 2]) -> [f32; 2] {
    [0, 1].map(|i| {
        let free = (screen[i] - size[i]).max(1.0);
        (pos[i] / free).clamp(0.0, 1.0)
    })
}

/// `pos` (top left of a `size` box) snapped to the screen's edges and
/// centre and to the edges of `others`; and the guide lines it snapped to
/// (x for vertical lines, y for horizontal ones).
pub fn snap(pos: [f32; 2], size: [f32; 2], others: &[[f32; 4]], screen: [f32; 2]) -> ([f32; 2], Vec<(usize, f32)>) {
    let mut out = pos;
    let mut guides = Vec::new();
    for axis in 0..2 {
        // Candidate lines and which edge of the box (0 start, 1 middle, 2 end) meets them.
        let mut lines = vec![0.0, screen[axis] / 2.0, screen[axis]];
        for o in others {
            let (start, len) = (o[axis], o[axis + 2]);
            lines.extend([start, start + len / 2.0, start + len]);
        }
        let edges = [0.0, size[axis] / 2.0, size[axis]];
        let mut best: Option<(f32, f32, f32)> = None; // (distance, new start, line)
        for line in &lines {
            for e in edges {
                let d = (pos[axis] + e - line).abs();
                if d <= SNAP && best.is_none_or(|b| d < b.0) {
                    best = Some((d, line - e, *line));
                }
            }
        }
        if let Some((_, start, line)) = best {
            out[axis] = start;
            guides.push((axis, line));
        }
        out[axis] = out[axis].clamp(0.0, (screen[axis] - size[axis]).max(0.0));
    }
    (out, guides)
}

/// What a click on the editor did.
#[derive(Clone, Debug, PartialEq)]
pub enum Click {
    /// A row of the list: turn this module on or off.
    Toggle(Module),
    /// A module's box: it is held now.
    Grab(Module),
    /// A right click on a box: back to its first place.
    Reset(Module),
    Nothing,
}

#[derive(Default)]
pub struct Editor {
    pub open: bool,
    pub close_asked: bool,
    /// The screen's size as the last frame drew it.
    pub size: [f32; 2],
    /// The module held and where in its box it was taken.
    held: Option<(Module, [f32; 2])>,
    guides: Vec<(usize, f32)>,
}

fn inside(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[1] >= r[1] && p[0] < r[0] + r[2] && p[1] < r[1] + r[3]
}

impl Editor {
    /// A press at `at` with `button` (0 left, 1 right); `boxes` are where
    /// the HUD modules are now.
    pub fn press(&mut self, at: [f32; 2], button: i32, boxes: &[(Module, [f32; 4])]) -> Click {
        if let Some(i) = list_row(at) {
            return if button == 0 { Click::Toggle(ALL[i]) } else { Click::Nothing };
        }
        let Some((m, r)) = boxes.iter().rev().find(|(_, r)| inside(*r, at)) else {
            return Click::Nothing;
        };
        if button == 1 {
            return Click::Reset(*m);
        }
        self.held = Some((*m, [at[0] - r[0], at[1] - r[1]]));
        Click::Grab(*m)
    }

    /// The pointer moved to `at` with the button down: the held module's
    /// new place as stored fractions.
    pub fn drag(&mut self, at: [f32; 2], boxes: &[(Module, [f32; 4])]) -> Option<(Module, [f32; 2])> {
        let (m, grab) = self.held?;
        let size = boxes.iter().find(|(b, _)| *b == m).map_or([20.0, 10.0], |(_, r)| [r[2], r[3]]);
        let others: Vec<[f32; 4]> = boxes.iter().filter(|(b, _)| *b != m).map(|(_, r)| *r).collect();
        let (pos, guides) = snap([at[0] - grab[0], at[1] - grab[1]], size, &others, self.size);
        self.guides = guides;
        Some((m, fraction(pos, size, self.size)))
    }

    pub fn release(&mut self) {
        self.held = None;
        self.guides.clear();
    }

    pub fn holding(&self) -> Option<Module> {
        self.held.map(|h| h.0)
    }

    /// The editor over the modules (which the hook draws first): a dim
    /// screen, a frame and name on each box, the snap guides, the list.
    pub fn draw(&self, boxes: &[(Module, [f32; 4])], enabled: &dyn Fn(Module) -> bool, mouse: [f32; 2]) -> Vec<Draw> {
        let [w, h] = self.size;
        let mut out = vec![Draw::Fill([0.0, 0.0, w, h], 0x5000_0000)];
        for (m, r) in boxes {
            let hot = self.holding() == Some(*m) || (self.held.is_none() && inside(*r, mouse));
            let c = if hot { 0xFF55_FFFF } else { 0x90FF_FFFF };
            let [x, y, bw, bh] = *r;
            out.push(Draw::Fill([x - 1.0, y - 1.0, bw + 2.0, 1.0], c));
            out.push(Draw::Fill([x - 1.0, y + bh, bw + 2.0, 1.0], c));
            out.push(Draw::Fill([x - 1.0, y, 1.0, bh], c));
            out.push(Draw::Fill([x + bw, y, 1.0, bh], c));
            if hot {
                out.push(Draw::Fill(*r, 0x2055_FFFF));
                out.push(Draw::Text(m.name().into(), [x, (y - 10.0).max(0.0)], 0x55FFFF, true));
            }
        }
        for (axis, line) in &self.guides {
            let rect = if *axis == 0 { [*line - 0.5, 0.0, 1.0, h] } else { [0.0, *line - 0.5, w, 1.0] };
            out.push(Draw::Fill(rect, 0xC0FF_55FF));
        }
        // The list.
        out.push(Draw::Fill([0.0, 0.0, LIST_W, 16.0 + ALL.len() as f32 * ROW + 26.0], 0xD010_1218));
        out.push(Draw::Text("Arena HUD".into(), [4.0, 4.0], 0x55FFFF, true));
        for (i, m) in ALL.iter().enumerate() {
            let y = 16.0 + i as f32 * ROW;
            let on = enabled(*m);
            if list_row(mouse) == Some(i) {
                out.push(Draw::Fill([2.0, y, LIST_W - 4.0, ROW - 1.0], 0x40FF_FFFF));
            }
            let mark = if on { "■" } else { "□" };
            out.push(Draw::Text(format!("{mark} {}", m.name()), [5.0, y + 1.0], if on { 0x7BE07B } else { 0x8A8A8A }, true));
        }
        let y = 18.0 + ALL.len() as f32 * ROW;
        out.push(Draw::Text("Sürükle: taşı".into(), [5.0, y], 0x9AA0AA, false));
        out.push(Draw::Text("Sağ tık: ilk yerine".into(), [5.0, y + 10.0], 0x9AA0AA, false));
        out
    }
}

/// The list row under `at`, if any.
fn list_row(at: [f32; 2]) -> Option<usize> {
    if at[0] < 0.0 || at[0] >= LIST_W || at[1] < 16.0 {
        return None;
    }
    let i = ((at[1] - 16.0) / ROW) as usize;
    (i < ALL.len()).then_some(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_round_trip_through_fractions() {
        let screen = [480.0, 270.0];
        let size = [40.0, 20.0];
        let f = fraction([110.0, 50.0], size, screen);
        assert_eq!(at(f, size, screen), [110.0, 50.0]);
        assert_eq!(at([1.0, 1.0], size, screen), [440.0, 250.0]);
    }

    #[test]
    fn boxes_snap_to_edges_centre_and_each_other() {
        let screen = [480.0, 270.0];
        let (p, g) = snap([3.0, 100.0], [40.0, 20.0], &[], screen);
        assert_eq!(p, [0.0, 100.0]);
        assert_eq!(g, vec![(0, 0.0)]);
        let (p, _) = snap([218.0, 50.0], [40.0, 20.0], &[], screen);
        assert_eq!(p[0], 220.0, "centred");
        let other = [[100.0, 80.0, 50.0, 12.0]];
        let (p, _) = snap([101.0, 94.0], [40.0, 20.0], &other, screen);
        assert_eq!(p, [100.0, 92.0], "left edges meet, top under its bottom");
        let (p, g) = snap([40.0, 40.0], [40.0, 20.0], &other, screen);
        assert_eq!(p, [40.0, 40.0]);
        assert!(g.is_empty());
    }

    #[test]
    fn the_list_toggles_and_boxes_drag() {
        let mut e = Editor { size: [480.0, 270.0], ..Editor::default() };
        assert_eq!(e.press([10.0, 16.0 + ROW + 2.0], 0, &[]), Click::Toggle(ALL[1]));
        let boxes = [(Module::Fps, [200.0, 100.0, 40.0, 14.0])];
        assert_eq!(e.press([210.0, 105.0], 1, &boxes), Click::Reset(Module::Fps));
        assert_eq!(e.press([210.0, 105.0], 0, &boxes), Click::Grab(Module::Fps));
        let (m, f) = e.drag([310.0, 155.0], &boxes).unwrap();
        assert_eq!(m, Module::Fps);
        assert_eq!(at(f, [40.0, 14.0], e.size), [300.0, 150.0]);
        e.release();
        assert!(e.drag([0.0, 0.0], &boxes).is_none());
    }
}
