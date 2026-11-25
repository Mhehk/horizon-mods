# Arena HUD — 16.10.2

Horizon (Rust ile yazılmış Minecraft 26.3) için PvP HUD native modu. Yazar: mihehk. Lisans: MIT.

Modül listesi Dawn Client'ınkiyle birebir ([dawn.gg/pvp-client](https://dawn.gg/pvp-client)).
Kodu sıfırdan yazıldı; Dawn, Feather ya da Lunar'ın kodu, adı veya görselleri kullanılmadı.
Dawn her modülün ayarlarını herkese açık bir yerde yayınlamadığı için ayarlar Lunar Client'ın
Keystrokes yazısına ve PvP istemcilerinin alışılmış düzenine göre seçildi. Dawn'ın ayar
ekranlarının görüntüleri gelince birebir hale getirilecek.

## Modüller

| Modül | Gösterdiği | Ayarlar |
| --- | --- | --- |
| Keystrokes | WASD, LMB/RMB (CPS'li), boşluk | WASD, fare, CPS, boşluk, oklar, renk geçişi (ms), kutu boyu, basılı renkler |
| CPS | Saniyedeki tıklama (son 1000 ms) | sol / sağ / ikisi, yazı |
| Mousestrokes | Fare hareketi (kameranın dönüşü) | boyut, hassasiyet, nokta rengi |
| Reach Display | Son vuruşun mesafesi (gözden hedefin kutusuna en yakın nokta) | ondalık, gösterme süresi, birim |
| Combo Display | Arka arkaya vuruş | bitme süresi (varsayılan 2 sn), sıfırken gizle, yazı |
| Hitbox | Varlıkların çerçevesi, göz hizası | renk, kalınlık, yalnızca oyuncular, göz çizgisi, menzil |
| Toggle Sprint | Koşma tuşu basınca açılır/kapanır | koşmayı sabitle, durum yazısı |
| Armor Status | Giyilen zırh ve eldeki eşya | eldeki eşya, alt alta |
| Potion Effects | Etkin iksirler | alt alta |
| Ping | Sunucu gecikmesi | birim |
| FPS | Saniyedeki kare (kendi sayar) | yazı |
| Block Overlay | Bakılan bloğun çerçevesi ve yüzleri | çerçeve, renk, kalınlık, dolgu, dolgu rengi |

Dawn'ın listesinde olmayan, bizim eklediğimiz yedi modül:

| Modül | Gösterdiği | Ayarlar |
| --- | --- | --- |
| Target HUD | Son vurduğun rakip: ad, can (renkli), can çubuğu, zırh puanı, uzaklık, elindeki eşya | gösterme süresi, can çubuğu, eşya, uzaklık, zırh |
| Speed | Saniyedeki blok (son 0,5 sn; ışınlanma sayılmaz) | ondalık, dikey hızı da say, birim |
| Coordinates | X Y Z, baktığın yön, Nether/Overworld karşılığı | ondalık, yön, diğer boyut |
| Session Stats | Toplam vuruş, en uzun kombo, ortalama reach | her satır ayrı açılıp kapanır · `/arena stats temizle` sıfırlar |
| Server Address | Bağlı olduğun sunucu (tek oyunculuda "Singleplayer") | portu göster, ön yazı |
| Session Time | Bu sunucuya / dünyaya girdiğinden beri geçen süre | ön yazı |
| Reach History | Son vuruşların mesafesi, en yenisi başta, Reach'in renkleriyle | kaç vuruş (1-10), ondalık, alt alta |

Her HUD modülünde ayrıca: açık/kapalı, arka plan, arka plan rengi, yazı rengi, `[ ]` içinde.

Zoom, Perspective (Freelook) ve Voice zaten ayrı modlar olarak var: Zoomify, Freelook ve
ÖzgürKamera, Voice Chat. Nick Hider ve Tier Tagger henüz yok (aşağıya bak).

## Kullanım

- **Sağ Alt:** HUD editörü. Modülü sürükleyerek taşı. Ekran kenarlarına, ortaya ve diğer
  modüllerin kenarlarına 4 piksel yaklaşınca yapışır, yapıştığı çizgi gösterilir. Sağ tık
  modülü ilk yerine koyar. Soldaki listeden modüller açılıp kapanır. Escape ya da Sağ Alt kapatır.
- **`/arena`:** modül listesi · `/arena <modül>` aç/kapat · `/arena <modül> ayarlar` ·
  `/arena <modül> <ayar> <değer>` (ör. `/arena keystrokes box 22`, `/arena cps buttons left`,
  `/arena hitbox color FFFF5555`) · `/arena <modül> sifirla` (yerini sıfırla) ·
  `/arena stats temizle` (oturum istatistiklerini sıfırla).
- Konumlar ekranın boş alanının oranı olarak saklanır, pencere boyutu değişince modül
  aynı tarafta kalır.

## Nasıl hesaplanıyor

- **CPS:** saldırı / kullan tuşunun her karede basılıp basılmadığına bakılır, inişler
  sayılır, son 1 saniyedekiler gösterilir.
- **Reach:** saldırı paketi gönderilirken gözden hedefin vuruş kutusunun en yakın noktasına
  olan mesafe ölçülür. Hedef 500 ms içinde hasar alırsa vuruş sayılır ve gösterilir.
- **Combo:** onaylanan vuruşlar art arda sayılır. Sen hasar alınca ya da 2 saniye vuruş
  olmayınca biter.
- **FPS:** HUD her karede çizildiği için kareler sayılır.

## Doğrulanmayanlar

Hiç derlenmedi. Horizon'un kaynağı herkese açık olmadığı için bazı oyun bilgileri tahmin
edildi, hepsi `state.rs`'teki "Horizon guesses" bölümünde:

- **Ping:** sekme listesindeki kendi gecikmen (`tab_players`, `session.uuid`, `latency`).
- **Armor Status / Potion Effects:** oyuncunun varlık listesindeki kaydı. Zırh yuvalarının
  adları tahmin, dayanıklılık ve iksir süreleri bilinmiyor; şimdilik yalnızca adlar görünür.
- **Toggle Sprint:** `player.sprinting` alanını açmak; oyunun bunu koşma sayıp saymadığı
  denenmedi.
- **Hitbox / Reach:** her varlığın kutusu oyuncu boyunda (0.6 x 1.8) alınıyor.

Doğrulanmış olanlar (başka modlar kullanıyor): tuş durumları, saldırı ve hasar paketleri,
varlık listesi, HUD ve dünya çizimi, mod ekranı.

Henüz yok:
- **Nick Hider:** ad etiketi, sohbet ve sekme listesi yazılarını değiştiren bir hook ister.
- **Tier Tagger:** Vanilla PvP Tierlist'in verisine internetten bağlanmayı ister.
- **Boyutlandırma:** Horizon'un HUD'unda büyütülmüş yazı çizme işlevi bilinmiyor.

## Paketleme

Eksen'in `tools/package.mjs` betiğiyle aynı biçimde `arena.hnmod`; derleme betiği
`.build-tools/horizon-build-mods.ps1` içinde `arena` olarak.