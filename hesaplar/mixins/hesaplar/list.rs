// Hesaplar's row in Options > Mods, with its key.
fn hesaplar_listed(mods: &mut Vec<NativeMod>) {
    use crate::game::hesaplar as m;
    mods.push(NativeMod {
        id: m::ID.into(),
        title: format!("{} — {}", m::NAME, m::VERSION),
        description: m::DESCRIPTION.into(),
        keys: m::KEYS.iter().map(|k| [k.0.to_string(), k.1.to_string(), k.2.to_string()]).collect(),
    });
}