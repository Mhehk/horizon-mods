// Older Servers' row in Options > Mods.
fn olderservers_listed(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: "olderservers".into(),
        title: "Older Servers — mhv1".into(),
        description: "1.20.2–26.2 sunucularına (ör. Fabric 1.21.11 oyuncularının Simple Voice Chat'li sunucularına) Horizon'un kendi sürüm çeviricisiyle bağlanır; sürüm ping ile otomatik bulunur.".into(),
        keys: Vec::new(),
    });
}
