// BetterHitreg's row in Options > Mods, with its keys: Open Hitreg Menu (H),
// Switch Hand and the score keys (the arrows).
fn betterhitreg_listed(mods: &mut Vec<NativeMod>) {
    let mut keys = vec![["menu".into(), "Open Hitreg Menu".into(), "key.keyboard.h".into()]];
    keys.extend(crate::game::remote::betterhitreg_fight::keys());
    mods.push(NativeMod {
        id: "betterhitreg".into(),
        title: "BetterHitreg — mhv2".into(),
        description: "Vuruşu sunucudan önce gösterir: kızarma, vuruş sesi ve kritik partikülü anında oynar, sunucunun geç gelen kopyası atlanır. Uyarılar, ses susturma, gizleme, renkler, zemin/ızgara ve skor da var. Ayarlar: /hitreg veya H. Jass'ın BetterHitreg modunun Horizon portu (Apache-2.0).".into(),
        keys,
    });
}
