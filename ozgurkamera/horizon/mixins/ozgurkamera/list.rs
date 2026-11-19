// Özgür Kamera row in Options > Mods, with its key (F6 by default).
fn ozgurkamera_listed(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: "ozgurkamera".into(),
        title: "Özgür Kamera — mhv1.2".into(),
        description: "Karakterinden bağımsız, duvarlardan geçebilen serbest kamera.".into(),
        keys: vec![[
            "toggle".into(),
            "Özgür kamerayı aç / kapat".into(),
            "key.keyboard.f6".into(),
        ]],
    });
}
