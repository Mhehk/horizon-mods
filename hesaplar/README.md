# Hesaplar — 16.10.1

Horizon (Rust ile yazılmış Minecraft 26.3) için native mod: ana menüde Microsoft hesapları
arasında oyunu kapatmadan geçiş. Essential'ın Account Manager'ı gibi çalışır
([Essential yardım sayfası](https://sparkuniverse.freshdesk.com/support/solutions/articles/201000105913-account-manager)).
Essential'ın kodu, adı ya da görselleri kullanılmadı. Yazar: mihehk. Lisans: MIT.

Yalnızca oyunu satın almış gerçek Microsoft hesapları. Offline/cracked hesap ve token
yapıştırarak giriş **yok**.

## Kullanım (Essential'daki gibi)

- Ana menünün **sağ üstünde** hesap düğmesi durur: kafan ve adın. Basınca hesap listesi açılır.
- **Hesap değiştir:** listeden başka bir hesaba bas. Şu anki hesap yeşil ve ✔ işaretli.
- **Hesap ekle:** **+ Hesap ekle** → **Tarayıcıyı aç**. Tarayıcıda Microsoft'un giriş sayfası
  açılır; giriş yapınca "Giriş tamamlandı" yazar, oyuna dön. Hesap kaydedilir ve hemen o
  hesaba geçilir.
- **Hesap kaldır:** hesabın üstüne gel, çıkan **×**'e bas, **Kaldır** ile onayla.
- Yalnızca dünyanın dışında: sunucudayken ya da tek oyunculu dünyadayken hesap değiştirilemez.
- Listenin dışına ya da hesap düğmesine yeniden basmak listeyi kapatır.

## Bir kez yapılacak: Azure uygulama kimliği

Microsoft girişi bir uygulama kimliği ister. Modda hazır kimlik yok; başka bir uygulamanın
(launcher, Xbox, Essential) kimliği de kullanılmaz. Kendi kimliğini al:

1. [portal.azure.com](https://portal.azure.com) → **App registrations** → **New registration**.
   Hesap türü: **Personal Microsoft accounts only**.
2. **Redirect URI**: platform **Public client/native (mobile & desktop)**, adres `http://localhost`
   (port yazma; Microsoft localhost'ta her porta izin veriyor).
3. **Application (client) ID**'yi kopyala. Hesap düğmesi → **+ Hesap ekle** → **Azure kimliği**
   alanına bas, yapıştırmak yerine yaz, Enter.
4. Mojang yeni uygulamaların Minecraft'a giriş yapmasına onay veriyor:
   [aka.ms/mce-reviewappid](https://aka.ms/mce-reviewappid) formunu doldur. Onay gelene kadar
   giriş "Minecraft girişi başarısız" diye durur.

## Nasıl çalışıyor

- Giriş zinciri: Microsoft tarayıcı girişi (yetki kodu + PKCE, `http://localhost:<port>`'a
  dönüş) → Xbox Live → XSTS → Minecraft → profil. Ayrı bir iş parçacığında çalışır, menü donmaz.
- Tarayıcı, modun yalnızca bu giriş için açtığı yerel porta döner; port sadece bilgisayarın
  kendisinden erişilebilir (127.0.0.1 / ::1) ve 5 dakika sonra kapanır.
- İstekler Windows'un kendi `curl.exe`'siyle yapılır. Ayarlar stdin'den verilir, böylece
  hiçbir kod komut satırında görünmez.
- Hesap listesi (ad, UUID, Microsoft yenileme kodu) `%APPDATA%\horizon\hesaplar.dat`
  dosyasında, Windows DPAPI (CurrentUser) ile şifreli durur. Dosyayı yalnızca aynı Windows
  kullanıcısı açabilir; başka bilgisayara kopyalanırsa işe yaramaz.
- Şifren mod tarafından hiç görülmez; giriş Microsoft'un kendi sayfasında olur.
- Bir hesabın erişimini tamamen kaldırmak için Microsoft hesabının gizlilik ayarlarından
  uygulamanın iznini geri al.

## Essential'dan farklar ve doğrulanmayanlar

- **Kafa:** Essential oyuncunun yüzünü çizer. Horizon'da skin yüzünü çizen işlevi henüz
  bilmiyoruz; şimdilik UUID'ye göre renkli bir kare ve adın baş harfi var.
- **Hangi ekranlar:** Horizon'da ekranın adını veren bir işlev bilmediğimiz için düğme yalnızca
  ana menüde değil, dünya dışındaki tüm menü ekranlarında görünür.
- **Tahmin edilen iki işlev:** Horizon'un kaynağı gizli ve elimizdeki hiçbir mod girişe
  dokunmuyor. `state.rs`'in sonundaki `horizon_current` (şu anki hesap) ve `horizon_switch`
  (hesabı değiştir) tahmin; ilk derlemede hata verirse düzeltilecek yerler bunlar.
