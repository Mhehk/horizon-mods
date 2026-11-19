// HatLayers' row in Options > Mods.
fn hatlayers_listed(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: "hatlayers".into(),
        title: "Hat Layers — mhv1.2".into(),
        description: "TAB listesinde kafaları her sunucuda gösterir; şapka katmanını ve kafanın üstündeki küçük yıldızı yalnızca aktif chunk mesafesindeki oyunculara verir.".into(),
        keys: Vec::new(),
    });
}
