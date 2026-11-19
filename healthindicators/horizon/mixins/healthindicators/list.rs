// Health Indicators' row in Options > Mods, with its six keys.
fn healthindicators_listed(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: "healthindicators".into(),
        title: "Health Indicators — mhv1".into(),
        description: "Oyuncu ve yaratıkların üstünde can (kalp veya sayı) ve zırh gösterir; filtreler, hedef izleme ve hasar yönü göstergesi. Ayarlar: I veya /healthindicators. AdyTech99'un Health Indicators modunun Horizon portu (LGPL-3.0).".into(),
        keys: crate::game::remote::healthindicators::keys(),
    });
}
