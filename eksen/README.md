# Eksen — mhv1

Horizon (Rust ile yazılmış Minecraft 26.3) için yapı editörü native modu. Yazar: mihehk.
Kodu sıfırdan yazıldı; başka bir moddan kod, isim veya görsel içermez.

## Özellikler

- **Uzaktan seçim:** bakılan bloğu 256 blok mesafeden köşe yapma (J / K), temizleme (U)
- **3D seçim kutusu:** dünyada çerçeve, yarı saydam dolgu, boyut etiketi; köşeler kırmızı/mavi
- **HUD kutusu:** sağ üstte seçim boyutu, fırça, pano ve kuyruk durumu
- **İşlemler:** doldur, sil, değiştir, içi boş kutu, dış yüzey, duvar
- **Kopyala / yapıştır / çoğalt / kaydır** (`clone` ile)
- **Fırça:** H basılıyken bakılan yere küre boyar; `sadece <blok>` ile maske
- Büyük işler 32768 blokluk parçalara bölünür ve sırayla gönderilir

Komutlar vanilla `fill` / `clone` olarak sunucuya gider: OP veya hile izni gerekir.
Geri alma yok; büyük işlerden önce dünyayı yedekle.

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

`target/eksen-mhv1.hnmod` üretir ve `%APPDATA%\horizon\horizon-native-mods\eksen.hnmod` olarak kurar.
