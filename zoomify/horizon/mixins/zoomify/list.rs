// Zoomify's row in Options > Mods, with its keys (C, F6, and the scroll keys with keybind scrolling).
fn zoomify_listed(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: crate::game::fov::zoomify::ID.into(),
        title: "Zoomify — mhv1".into(),
        description: "A zoom mod with infinite customizability. Port of Zoomify by isXander (LGPL-3.0).".into(),
        keys: crate::game::fov::zoomify::keys(),
    });
}
