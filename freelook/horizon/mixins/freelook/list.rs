// Freelook's row in Options > Mods, with its keys (Left Alt, M).
fn freelook_listed(mods: &mut Vec<NativeMod>) {
    use crate::game::perspective::freelook as f;
    mods.push(NativeMod {
        id: f::ID.into(),
        title: "Freelook — mhv1".into(),
        description: "Look around your player without turning it. Port of Freelook by Celibistrial (AGPL-3.0).".into(),
        keys: f::KEYS.iter().map(|(n, l, d)| [(*n).into(), (*l).into(), (*d).into()]).collect(),
    });
}
