# Eksen — 16.10.2

Horizon (Rust ile yazılmış Minecraft 26.3) için yapı editörü native modu. Yazar: mihehk.
Kodu sıfırdan yazıldı; başka bir moddan kod, isim veya görsel içermez.

## Özellikler

- **Uzaktan seçim:** bakılan bloğu 256 blok mesafeden köşe yapma (J / K), temizleme (U)
- **Magic Select (G):** Axiom'un belgelediği davranışla, sıfırdan yazıldı. Tuşu basılı tutup
  bloklara bakarsın, bırakınca o türlerle bitişik bloklar tıklanan bloktan dışa doğru seçilir
  (limitle kesilince sekizyüzlü şekil oluşur). Ayarlar `/eksen sihirli` ile değişir:
  karşılaştırma (blok / durum / katı / herhangi), limit (varsayılan 100 000), menzil (boşluk
  atlama, 1–8), yalnızca yüzey, yalnızca yukarı / aşağı, yatay, köşeler. Seçim işlemi
  `/eksen islem ekle|cikar|yerine|kesisim` ile belirlenir. Doldur, sil ve değiştir bu seçimde
  satır satır çalışır.
  **Henüz çalışmıyor:** Horizon'un blok okuma işlevi bilinmediği için `state.rs` içindeki
  `GameBlocks` boş. Bağlanana kadar Magic Select bunu sohbette söyler.
- **3D seçim kutusu:** dünyada çerçeve, yarı saydam dolgu, boyut etiketi; köşeler kırmızı/mavi
- **HUD kutusu:** sağ üstte seçim boyutu, fırça, pano ve kuyruk durumu
- **İşlemler:** doldur, sil, değiştir, içi boş kutu, dış yüzey, duvar
- **Kopyala / yapıştır / çoğalt / kaydır** (`clone` ile)
- **Fırça:** H basılıyken bakılan yere küre boyar; `sadece <blok>` ile maske
- Büyük işler 32768 blokluk parçalara bölünür ve sırayla gönderilir

Komutlar vanilla `fill` / `clone` olarak sunucuya gider: OP veya hile izni gerekir (sunucuda
AxiomPaper varsa araçların düzenlemeleri eklentiden gider, OP gerekmez). Geri alma yalnızca
araçların değişikliklerinde var; büyük işlerden önce dünyayı yedekle.

## Axiom'un davranışıyla yeniden yazılan özellikler

Hepsi Axiom'un resmi belgelerindeki davranışa göre sıfırdan yazıldı
(`.build-tools/axiom-docs/axiom-docs.md`). Axiom'un kodu, görselleri ve adı kullanılmadı.

- **Araçlar** (`/eksen arac`): seçim (Kutu, Serbest, Kement, Magic Select), yardımcı (Cetvel, Not),
  boyama (`tools_paint.rs`), çizim ve modelleme (`tools_draw.rs`), sıvı, yükselti ve şekil
  değiştirme (`tools_terrain.rs`). B basılı tutulup bloklara bakılarak çizilir, bırakınca araç
  çalışır. N ikinci işlemi yapar (sil, alçalt …). Ayarlar `/eksen ayar`, aktif blok `/eksen blok`,
  metin `/eksen metin` ile verilir.
- **10. slot yapı araçları** (`builder.rs`): Taşı, Klonla, Çoğalt, Sür (Smear), Uzat/Kısalt
  (Extrude), Sil / Bağlı Sil (en yakın 128 aynı blok), Simetri noktası (aynalama + 2/4 katlı
  döndürme). Ctrl+R döndürür, Ctrl+F baktığın eksende aynalar. Merdiven yönü, kütük ekseni ve çit
  kenarları gibi blok durumları da döner. "Var olanı koru" ve "havayı kopyala" bayrakları var.
- **Geri al / yinele:** Ctrl+Z / Ctrl+Y (`/eksen geri`, `/eksen ileri`). Son 64 değişiklik tutulur.
- **Yetenekler** (menünün sol tarafı, `/eksen yetenek`): Buldozer, Değiştirme Modu (R), Zorla
  Koy, Güncellemesiz, Kurcala, Sonsuz Menzil, Hızlı Koy, Havada Koy, Duvardan Geçme.
- **Uçuş hızı ve araç kutusu** (`/eksen ucus`): uçuş hızı %1–999 (`+`/`-` ile 10'ar), uçuş yönü
  (yatay/kamera), ivme, sonsuz menzil sınırı (5–100), sıvı opaklığı, en düşük parlaklık, türü
  koruyarak değiştirme.

- **Editör ekranı (Sağ Shift):** fare serbest kalır. Üstte sekmeler (Araçlar, Yapı, Yetenekler,
  Seçim), solda liste, sağda etkin aracın ayarları, altta aktif blok ve metin alanı ile son
  mesajlar. Ortadaki dünyaya sol tıklayıp sürükleyince araç imlecin altındaki bloklarda çalışır,
  sağ tık ikinci işlemi yapar. İmlecin altındaki blok beyaz çerçeveyle gösterilir. Ctrl+Z/Y/R/F
  ve G (Magic Select) ekranda da çalışır. Sağ Shift ya da Escape kapatır. Her düğme bir
  `/eksen` komutunu çalıştırır, yani ekran ve sohbet komutları aynı yoldan gider (`ui.rs`).
- **AxiomPaper uyumu** (`axiom.rs`, `/eksen axiom`): sunucuda Axiom'un Paper eklentisi
  (AxiomPaper, MIT) varsa Eksen onun el sıkışmasına cevap verir ve düzenlemeleri eklentinin
  kendi yolundan gönderir: bölüm bölüm blok tamponu (`set_buffer`), uzak/havada tek blok
  (`set_block`) ve uçuş hızı (`set_fly_speed`). Eklentinin izinleri (`axiom.build.section` …),
  hız sınırı ve koruma eklentileri (WorldGuard, PlotSquared, CoreProtect) uygulanır. OP gerekmez,
  `axiom.use` izni yeter. Eklenti yoksa ya da izin vermiyorsa her şey eskisi gibi komutla gider.
  Protokol AxiomPaper'ın kaynağından okundu (API sürümü 10).

### Henüz çalışmayanlar / doğrulanmayanlar

Horizon'un kaynağı herkese açık olmadığı için bazı oyun işlevleri tahmin edildi. Mod hiç
derlenmedi:

- **Blok okuma** (`state.rs` → `GameBlocks`): `game.world.block(x, y, z)` ve `is_air` başka bir
  modda (BetterHitreg) kullanıldığı için doğru. Bloğun adını, katılığını ve kimliğini veren
  `state_string`, `is_solid`, `parse_state` ve `horizon_protocol::PROTOCOL_VERSION` ise tahmin.
  İlk derleme hata verirse düzeltilecek yer yalnızca bu dört satır.
- **Editör ekranı:** ekran hook'ları VoiceChat'inkilerle aynı. Kamera `game.locator_camera`'dan
  okunuyor. Ekranın arkasında dünyanın çizilip çizilmediği denenmedi.
- **Uçuş hızı:** AxiomPaper'a bildiriliyor (sunucu tarafında uçuş hızı değişir). İstemcinin kendi
  hareketini hızlandırmak için Horizon'un hareket hook'u hâlâ gerekiyor.
- **Oyuncu hareketi ve çizim:** uçuş yönü ve ivmesi, duvardan geçme, sıvı opaklığı ve
  parlaklık şimdilik yalnızca ayar olarak tutuluyor.
- **Blok etkileşimi:** Değiştirme Modu, Zorla Koy, Hızlı Koy ve Kurcala, vanilla'nın kendi
  koymasını engelleyen bir hook ister.
- **Komutla çalışanlar:** Buldozer (her tick bir blok kırar), Sonsuz Menzil (vanilla menzilinin
  ötesine koyar), Havada Koy ve Güncellemesiz (`strict`). Fare tuşlarının hook'a
  `key.mouse.left/right` olarak geldiği varsayıldı, doğrulanmadı.

## Komutlar

`/eksen yardim` listeyi gösterir. Başlıcaları:
`doldur <blok>`, `degistir <eski> <yeni>`, `bos <blok>`, `dis <blok>`, `duvar <blok>`, `sil`,
`kopyala`, `yapistir`, `cogalt <adet> [yön]`, `kaydir <adet> [yön]`,
`firca <yarıçap> <blok> [sadece <blok>]`, `firca kapat`, `pos1`, `pos2`, `secim`, `durdur`.

## Uyumluluk

Horizon `nightly-source/2026-11-13-9f42064` sürümüne sabitlenmiştir (dünya çizimi için
henüz katalogda olmayan `native_world_primitives` kullanılır). Başka bir nightly için
`horizon-mixin.toml` ve `compatibility.json` içindeki `horizon_revision` güncellenip yeniden
denetlenmeli.

## Paketleme

```bash
node tools/package.mjs --install
```

`target/eksen-16.10.2.hnmod` üretir ve `%APPDATA%\horizon\horizon-native-mods\eksen.hnmod` olarak kurar.
