# OpSec — 16.10.1 (Horizon)

Aurick'in [OpSec](https://github.com/aurickk/OpSec) modunun Horizon portu (mihehk).
Lisans: GPL-3.0-only (`LICENSE`).

Horizon Fabric değil. Bu yüzden OpSec'in mod listesi, kanal ve çeviri anahtarı korumalarının
burada gizleyeceği bir şey yok. Bir sunucunun her istemciye hâlâ yapabileceği şey, onu kaynak
paketleriyle yoklamak. Port bu korumayı taşır:

- **TrackPack:** Sunucu kısa sürede tek bir adrese çok sayıda farklı paket hash'i gönderirse
  (5 saniyede 10 istek ve adres başına 3+ hash, OpSec'in eşikleriyle aynı), önbelleğinde hangi
  paketlerin olduğunu öğrenip seni sunucular arasında tanımaya çalışıyordur. Bundan sonraki
  paketler indirilmeden ve önbelleğe bakılmadan "indirilemedi" diye yanıtlanır.
- **Yerel ağ yoklaması:** İnternetteki bir sunucu seni kendi bilgisayarındaki ya da ağındaki bir
  adrese (127.0.0.1, 10.x, 192.168.x, 169.254.x, ::1, fc00::/7 …) yönlendirirse paket engellenir.
  Sunucunun kendisi yerel ağdaysa (LAN sunucusu) engellenmez.

Engellenen ilk pakette sohbete bir uyarı düşer. `/opsec` bu sunucuda kaç paketin engellendiğini
gösterir. Mod, Options > Mods'tan kapatılabilir.

## Bilinen sınırlar

- Yalnızca oyun (play) aşamasındaki paketler yakalanır. Sunucu paketi giriş sırasındaki
  yapılandırma (configuration) aşamasında gönderirse koruma devreye girmez.
- `guard.rs` içindeki `UNVERIFIED` işaretli adlar (`ResourcePackPush`, `ResourcePack`,
  `ResourcePackAction`) Horizon kaynağına bakılmadan Mojang adlarından tahmin edildi. Derleme
  hata verirse önce bunlar düzeltilmeli.
- Derlenmedi ve oyunda test edilmedi.
