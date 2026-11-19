# Zoomify — mhv1 (Horizon)

isXander'ın **Zoomify** modunun Horizon (Minecraft 26.3, Rust) portu. Lisans: LGPL-3.0-or-later (orijinal gibi, `LICENSE`).

## Kullanım
- **C** (basılı tut): zoom. Ayarlardan "Toggle" yapılabilir.
- Zoom yaparken **fare tekerleği**: adım adım daha fazla yakınlaştır / uzaklaştır.
- **F6**: ikincil zoom (yavaş, aç/kapa; HUD'u gizleyebilir). Not: Özgür Kamera da F6 kullanır; Options > Mods'tan birini değiştir.
- Ayarlar: Options > Mods > Zoomify > Settings veya `/zoomify`.

## Ayarlar (orijinalle aynı anahtarlar, `config/zoomify.json`)
- **Behavior:** Initial Zoom, Zoom In/Out Time, Zoom In/Out Transition, Affect Hand FOV, Enable Scroll Zoom, Scroll Step Count, Zoom Per Step, Scroll Zoom Smoothness, Remember Zoom Steps, Spyglass (Zoom Behavior, Overlay Visibility, Sound Behavior).
- **Controls:** Zoom Key Behavior, Keybind Scrolling (yeniden başlatma gerekir), Relative Sensitivity, Relative View Bobbing, Cinematic Camera.
- **Secondary Zoom:** Zoom Amount, Zoom In/Out Time, Hide HUD On Zoom.
- **Misc:** Default / OptiFine / OK Zoomer hazır ayarları.

Düğmeye tıkla: sonraki değer. **Shift**: önceki değer, **Ctrl**: 10 adım.

## Orijinalden farklar
- Ayar ekranı YACL yerine Horizon'un mod ayar sayfasında; önizleme animasyonu yok.
- Cinematic Camera'nın yumuşaklık yüzdesi açık/kapalı olarak uygulanır (Horizon'un kendi yumuşatması kullanılır).
