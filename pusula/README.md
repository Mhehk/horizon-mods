# Pusula — 16.10.1

Horizon (Rust ile yazılmış Minecraft 26.3) için minimap ve dünya haritası, tek mod. Yazar:
mihehk. Lisans: MIT.

Xaero's Minimap ve Xaero's World Map'in herkese açık sayfalarında anlatıldığı gibi çalışır,
ama kodu sıfırdan yazıldı. Xaero'nun modları "All Rights Reserved" lisanslı; kodları, adları
ve görselleri kullanılmadı.

## Minimap

- Köşede kare ya da daire harita. Varsayılan olarak baktığın yöne döner, istersen kuzey
  hep üstte kalır. Kenarda N (döndüğünde E, S, W da) yazar.
- Yakınlaştırma: `=` ve `-` (0.5 ile 4 piksel/blok arası).
- Radar: oyuncular beyaz (adlarıyla), düşman moblar kırmızı, diğerleri sarı noktalar.
- Waypointler renkli kare ve baş harfiyle; haritanın dışındakiler kenarda durur.
- Altında koordinatlar.
- **Mağara modu:** başının üstünde tavan varsa (yapraklar ve kütükler sayılmaz) harita
  ayaklarının çevresindeki zemini gösterir, köşede "Mağara" yazar. Nether'da tavan olduğu
  için kendiliğinden açılır. `cave` ayarıyla hep açık ya da hep kapalı yapılabilir.

## Dünya haritası (M)

- Gezdiğin her yer kaydedilir. Sunucuya, dünyaya ve boyuta göre ayrı ayrı tutulur.
- Sürükleyerek kaydır, tekerlekle imlecin olduğu yere yakınlaş (1:16 ile 8:1 arası).
  Yakından bakınca chunk çizgileri görünür.
- Altta imlecin altındaki bloğun X, Y, Z'si ve yakınlık.
- **Sağ tık:** "Waypoint oluştur" ya da "Işınlan (/tp)". Işınlanma `/tp` komutunu gönderir,
  yani sunucuda yetkin yoksa sunucu reddeder.
- Sağda waypoint listesi: tıkla, harita oraya gitsin; × ile sil. "Bana ortala" haritayı yine
  seni takip eder hale getirir.
- `M` ya da Escape kapatır.

## Waypointler

- `B`: bulunduğun yerde yeni waypoint (ad yaz, renk kutusuna tıklayıp rengi değiştir,
  Enter ya da Kaydet).
- Dünyada adları ve uzaklıklarıyla görünürler (`in_world` ayarı).
- Ölünce ölüm noktası "Ölüm" adıyla eklenir, bir sonraki ölümde yerine yenisi gelir.
- `/pusula wp ekle [ad]`, `/pusula wp sil <ad>`, `/pusula wp liste`.

## Ayarlar

`/pusula ayar` hepsini gösterir, `/pusula ayar <ayar> <değer>` değiştirir:

| Ayar | Varsayılan | |
| --- | --- | --- |
| `minimap` | acik | minimap açık/kapalı (`/pusula minimap` da açar/kapatır) |
| `size` | 100 | minimap boyu, 50-200 piksel |
| `zoom` | 1 | piksel/blok: 0.5, 1, 1.5, 2, 3, 4 |
| `shape` | kare | kare / daire |
| `rotate` | acik | harita baktığın yöne döner |
| `corner` | sol-ust | sol-ust / sag-ust / sol-alt / sag-alt |
| `coords` | acik | koordinatlar |
| `radar` | acik | oyuncular ve moblar |
| `names` | acik | oyuncu adları |
| `cave` | oto | mağara modu: oto / acik / kapali |
| `in_world` | acik | waypointler dünyada |
| `detail` | 80 | minimapin bir kenarındaki kare sayısı (32-128) |
| `speed` | 2 | her karede taranan chunk (1-16) |

**Bir sunucuda birden fazla dünya varsa** (lobi, survival …): `/pusula dunya survival` o
dünyanın haritasını ayırır, `/pusula dunya` varsayılana döner.

## Nasıl çalışıyor

- Her karede oyuncunun çevresindeki (12 chunk'a kadar) yüklü chunk'lardan birkaçı, en
  yakındakiler önce olmak üzere taranır. Her sütunun en üstteki bloğu bulunur; su varsa
  altındaki zemin ve suyun derinliği de. Yanındaki chunk'lar 1,5 saniyede bir, uzaktakiler
  20 saniyede bir yeniden taranır.
- Renkler bloğun adından gelir (kendi renk tablomuz, ör. çimen yeşil, kum açık sarı). Su
  derinleştikçe koyulaşır, yükseklik farkı gölge olarak görünür.
- Harita, Horizon'un bu moda verdiği veri klasöründe (`native_data_dir`, Voice Chat'in de
  kullandığı) saklanır: her 512 x 512 blokluk
  bölge için bir `.psl` dosyası ve her dünya için `waypoints.txt`. 30 saniyede bir ve dünya
  değişince kaydedilir.
- Harita dikdörtgenlerle çizilir; yan yana aynı renkteki kareler tek dikdörtgen olur.
  Horizon'un mod dokusu (`horizon/modimage`) tek ve ortak olduğu için kullanılmadı,
  ShulkerBoxTooltip onu kullanıyor.

## Doğrulanmayanlar

Hiç derlenmedi. Horizon'un kaynağı herkese açık olmadığı için iki şey tahmin, `state.rs`'te
işaretli:

- Bir blok durumunun kimliği (`state.id()`), renkleri önbelleğe almak için. Eksen'in aynı
  tahmini. Blok adı (`state_string`) da Eksen'in tahmini.
- Ölüm noktası: oyuncunun mob listesindeki kendi kaydının canı (Arena HUD'un tahmini).

Doğrulanmış olanlar (başka modlar kullanıyor): blok ve chunk okuma, oyuncu konumu ve yönü,
moblar ve oyuncu adları, sunucu adresi, kamera, komut gönderme, HUD, mod ekranı ve tuşlar.

Henüz yok: biyom adı, mağara katmanlarının kaydedilmesi (mağara haritası yalnızca o oturumda
tutulur), dokulardan renk (Xaero blok dokularının ortalama rengini kullanır, Horizon'da doku
okuma işlevi bilinmiyor), waypoint düzenleme ekranı.

## Paketleme

`node tools/package.mjs` → `target/pusula-16.10.1.hnmod`; derleme betiğinde `pusula` olarak.
