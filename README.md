# horizon-mods (mihehk)

Horizon (Minecraft 26.3'ün Rust yeniden yazımı) için native mod portları. Hepsi
`horizon-mixins` paketleri ve **nightly-2026-11-13-9f42064**'e sabitli; o sürümün
`horizon-client.exe` dosyasına derlenir. 18 Kasım (0b14f8a) ve sonrasında mod sistemi
değiştiği için orada çalışmazlar.

| Klasör | Mod | Sürüm | Kaynak / lisans |
| --- | --- | --- | --- |
| `eksen` | Eksen (Axiom araçları, editör ekranı, AxiomPaper uyumu) | 16.10.2 | mihehk |
| `shulkerboxtooltip` | ShulkerBoxTooltip | — | MisterPeModder (Yanis Guaye), MIT |
| `noreloadscreen` | Remove Reloading Screen (RRLS) | mhv2 | dima_dencep, OSL-3.0 |
| `hatlayers` | HatLayers | mhv1.2 | mihehk, MIT |
| `ozgurkamera` | ÖzgürKamera | mhv1.2 | mihehk, MIT |
| `freelook` | Freelook | mhv1 | Celibistrial, AGPL-3.0 |
| `zoomify` | Zoomify | mhv1 | isXander, LGPL-3.0 |
| `betterhitreg` | BetterHitreg | mhv2 | Jass, Apache-2.0 |
| `voicechat` | Voice Chat (Simple Voice Chat uyumlu) | mhv2 | Nirokx & BnCatt, MIT |
| `olderservers` | Older Servers | mhv1 | mihehk |
| `healthindicators` | Health Indicators | mhv1 | AdyTech99, LGPL-3.0 |
| `opsec` | OpSec (kaynak paketi koruması) | 16.10.1 | Aurick, GPL-3.0-only |

Voice Chat, Simple Voice Chat'in kodunu veya resimlerini **içermez**; görselleri oyuncunun
kendi SVC jar'ından çalışma anında okur (`voicechat/KURULUM.md`).

## Sürümleme

Yukarıdaki modlar eski `mhvN` sürümlerinde kalır. Bundan sonra yapılan her mod
**16.10.1** ile başlar:

- Hata düzeltmesi (bug fix) gibi küçük değişikliklerde sona bir alt numara eklenir: `16.10.1.2`, `16.10.1.3` …
- Büyük bir değişiklikte orta numara artar: `16.10.1` → `16.10.2`.

## Derleme

`build-tools/horizon-build-mods.ps1` (yollar `cmod` klasörüne göre). Kaynaklar `cmod`
altındaki proje klasörlerindedir; bu depoyu güncellemek için `.\sync.ps1`.
