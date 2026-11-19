# Voice Chat — mhv2 (Horizon)

Simple Voice Chat sunucularıyla konuşmayı sağlayan, **resmî olmayan** Horizon istemcisi. Nirokx ve BnCatt'in
MIT lisanslı, sıfırdan yazılmış Horizon Voice Chat 1.3.0'ına dayanır (`LICENSE`). mhv2'de ekranlar, tuşlar,
ayarlar ve HUD Simple Voice Chat 2.6.x ile birebir aynı düzende.

## SVC'nin görüntüsü
Simple Voice Chat'in lisansı "All Rights Reserved" olduğu için bu mod onun resimlerini **içermez**. Bunun yerine
oyun açılınca senin bilgisayarındaki kendi SVC jar'ını bulur ve ekran arka planlarını ve ikonları oradan okur:
- önce `%APPDATA%\horizon\horizon-mods\.horizon-mod-data\voicechat\` içindeki `voicechat*.jar` (ya da `svc\assets\...`),
- sonra Dawn/Feather, Modrinth, Prism, CurseForge ve `.minecraft\mods` klasörleri (en yeni sürüm seçilir).
Bu bilgisayarda Dawn'daki `voicechat-fabric-1.21.11-2.6.22.jar` bulunuyor. Jar yoksa modun kendi ikonları çizilir.

## Kurulum (bu bilgisayarda yapıldı)
- Mod, aktif sürümün `horizon-client.exe` dosyasına derlendi (`.build-tools/horizon-build-mods.ps1`).
- `opus.dll` resmî libopus 1.5.2 kaynağından (SHA256 doğrulandı) launcher'ın MinGW'siyle derlendi ve
  `%APPDATA%\horizon\horizon-mods\.horizon-mod-data\voicechat\` klasörüne lisansıyla (`COPYING.libopus`) kondu.
  Kopyası: `dist/`.
- `rnnoise.dll` (gürültü bastırma ve otomatik ses algılama): xiph/rnnoise v0.2 resmî sürüm arşivinden
  (SHA-256 `90fce4b0…0d37`, 2 294 308 bayt) launcher'ın MinGW'siyle derlendi, aynı klasöre lisansıyla
  (`COPYING.rnnoise`, BSD-3) kondu. Kopyası: `dist/`. Testte cızırtı −74 dB bastırıldı, ton −0,2 dB kaldı.

## Fabric (Java) oyuncularıyla konuşmak
- Sunucuda **Simple Voice Chat** eklentisi veya modu olmalı (UDP portu, varsayılan 24454, açık).
- Bu istemci SVC'nin uyumluluk sürümü 20 (2.6.x, örn. `voicechat-fabric-1.21.11-2.6.22`) ve 21 (2.7)
  protokollerini konuşur; ikisini de otomatik dener. Yerel Fabric 1.21.11 + SVC 2.6.22 sunucusunda denendi:
  sunucu "successfully connected to voice chat" diyor.
- 1.21.x / 26.1–26.2 sunucularına girmek için **Older Servers — mhv1** modu açık olmalı.

## SVC ile aynı olanlar
- Tuşlar (SVC varsayılanları): V menü, M mikrofonu sustur, N sesli sohbeti kapat, H ikonları gizle; bas-konuş,
  fısıltı, ayarlar, grup, kayıt ve ses ayarı tuşları atanmamış gelir.
- İlk açılışta kurulum sihirbazı: tanıtım/atla, mikrofon, hoparlör, yöntem, bas-konuş tuşu veya ses eşiği, bitir.
- Ana ekran (195×76), ayarlar (248×219: ses %0–300, Otomatik/Manuel kazanç −40…24 dB, gürültü bastırma,
  mikrofon testi, yöntem, bas-konuş tuşu ya da Otomatik/Manuel ses algılama + eşik, 3D ses, cihazlar),
  grup listesi (kilit, üye kafaları), grup oluştur, şifre, grup içi (kafalar, konuşma çerçevesi, kişi başı ses,
  grup HUD, ayrıl), ses ayarı (arama, "Other", kişi başı %0–400), cihaz seçimi.
- HUD: durum ikonu sol altta (16, −16), grup kafaları sol üstte 2× ölçek, isimlerin yanında konuşma/fısıltı/
  bağlantı yok/başka grup/kapalı ikonları.
- Sunucu izin verirse kayıt: oyuncu başına WAV, `voicechat\recordings\session-...` klasörüne.

## Farklar
- Kayıt SVC'nin MP3 yerine WAV yazar.
- Grup adını oyuncuların üstünde gösterme modun kendi ek özelliği (varsayılan kapalı).
