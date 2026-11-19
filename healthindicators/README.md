# Health Indicators — mhv1 (Horizon)

AdyTech99'un [Health Indicators](https://github.com/AdyTech99/HealthIndicators) modunun Horizon (Rust, MC 26.3) portu. Lisans: orijinal gibi **GNU LGPL v3** (`horizon/mixins/healthindicators/LICENSE`). Port: mihehk.

## Özellikler (orijinaldeki gibi)

- Oyuncu ve yaratıkların üstünde **kalp**, **sayı** (`13.0 / 20.0` veya `%`) ya da **dinamik** gösterge; sarı (absorption) kalpler, wither/zehir/donma ve hardcore kalpleri, rejenerasyon zıplaması.
- **Zırh puanları** (sağ Shift ile aç/kapa).
- Filtreler: pasif/düşman/oyuncu/kendin, kara/beyaz liste, sadece vurulanlar (süreli), sadece hasarlılar, sadece baktığın, mesafe, duvar arkasından gösterme.
- Görünüm: boyut, opaklık, satır başına kalp, ofset, ondalık, yüzde, renk, isimli varlıklarda yukarı kaydırma.
- **Hasar yönü göstergesi** (tek oyunculu).
- Tuşlar: ← göstergeyi aç/kapa, sağ Shift zırh, → (basılı) tüm filtreleri geçersiz kıl, ↑/↓ ofset, I ayarlar.
- Komutlar: `/healthindicators`, `offset <sayı>`, `indicator-type <heart|number>`, `monitor <isim>`, `stop-monitoring`.

Ayarlar: Seçenekler > Modlar > Health Indicators (4 bölüm: Visibility, Appearance, Messages & Commands, Damage Direction Indicators). Dosyalar: `config/health_indicators_config.json` ve `config/healthindicators.json` (orijinal anahtarlar). Liste (Filtered Entities) dosyada düzenlenir; butona basınca Not Defteri açılır.

## Farklar

- "Use vanilla textures" açıkken orijinal modun kendi eski kalp dokuları yerine oyunun kalp sprite'ları kullanılır (Horizon'da mod dokusu atlasa eklenemiyor).
- Yaratıkların absorption değeri sunucudan gelmediği için (vanilla'da da öyle) sarı kalpler yalnızca oyuncularda görünür.

## Derleme

`.build-tools/horizon-build-mods.ps1` (nightly-2026-11-13-9f42064'e sabit).
