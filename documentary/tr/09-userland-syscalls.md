# 09 — Userland ve Syscall

Userland, Rusty OS'un bir çekirdek olmayı bırakıp bir insanın kullandığı bir şey
olmaya başladığı yerdir. Çekirdekten ayrı olarak `core.bin` adlı düz bir ikiliye
derlenir, sabit bir düşük adrese yüklenir ve ring 3'te çalıştırılır — ayrıcalıksız,
donanıma doğrudan dokunamayan ve çekirdeğe yalnızca sistem çağrıları aracılığıyla
ulaşan.

## Başlangıç

Userland, bir yükleyicisi olan normal bir program yerine ham bir ikili olduğu için,
başka her şey çalışmadan önce kendi ortamını hazırlamak zorundadır. Userland'ın
`main`'indeki giriş noktası, iki temel şey yapan küçük bir assembly parçasıdır: BSS
bölümünü sıfırlar ve Rust'a çağrı yapmadan önce yığını hizalar.

BSS ayrıntısı gerçek bir savaş hikâyesidir. Düz bir ikilide BSS — sıfırla
ilklendirilen statiklerin alanı — dosyada saklanmaz. Soğuk bir önyüklemede bu
tesadüfen işe yaradı, çünkü makinenin RAM'i sıfırlanmış başlıyordu. Ama sıcak bir
yeniden başlatmada RAM hâlâ önceki oturumun çöpünü tutuyordu, dolayısıyla o
statikler — heap ayırıcısının kendi durumu da dahil — bozuk olarak açılırdı. Çözüm,
BSS'i userland'ın yaptığı ilk şey olarak, ona bağımlı olabilecek herhangi bir Rust
kodundan önce elle sıfırlamaktı. Birisi kapatmadan yeniden başlatana kadar görünmez
olan, sonra da şaşırtıcı hâle gelen türden bir hata.

Ortam sağlam olduğunda userland heap'ini ilklendirir, framebuffer bilgisini
çekirdekten alır, bir renderer oluşturur ve — kurulum yapılmadıysa — masaüstünü
kurmadan önce OOBE sihirbazını ve giriş ekranını çalıştırır.

## Syscall ABI'si

Çekirdekle her etkileşim bir sistem çağrısından geçer. Kural derli topludur: çağrı
numarası `rax`'e, argümanlar `rdi` ve `rsi`'ye gider ve `syscall` komutu atlamayı
yapar. Userland tarafında `syscall.rs`, her birini küçük bir işlevle sarar; derleyici
her çağrıyı ayrı olarak ele alsın ve komutun bozduğu yazmaçları korusun diye
dikkatle işaretlenmiştir.

Çağrılar, userland'ın ihtiyaç duyduğu her şeyi ve ihtiyaç duymadığı hiçbir şeyi
kapsamaz:

| # | Çağrı | Amaç |
|---|-------|------|
| 0 | print | ekrana metin yaz (erken hata ayıklama) |
| 2 | framebuffer al | ekran tabanı, boyut, stride, arka tampon |
| 3 | olay yokla | sıradaki klavye ya da fare olayını çek |
| 4 | saat al | gerçek zamanlı saati oku |
| 5 | güç | kapat ya da yeniden başlat |
| 6–7 | önbellekli okuma / renk ayarla | hızlı değer erişimi ve masaüstü rengi |
| 8–13 | dizin & dosya işlemleri | listele, oluştur, sil, yeniden adlandır, mkdir, taşı |
| 14–15 | kayıt listele / ayarla | kayıt satırı dök ya da ayarla |
| 16–17 | dosya oku / yaz | dosya içeriği giriş ve çıkışı |
| 18–20 | ses | açılış sesi çal, dosya çal, durdur |
| 21 | sistem bilgisi | RAM ve CPU istatistikleri |

Tekrar eden bir desen, değişken uzunluklu verinin sınırı nasıl geçtiğidir: yollar
ve dosya içerikleri, küçük bir uzunluk başlığıyla bir tampona paketlenir ve çekirdek
onları diğer tarafta açar. Mütevazı bir ABI, ama üzerine bir dosya yöneticisi, bir
metin editörü ve diğer her şeyi inşa etmeye yetiyor.

## Olay döngüsü ve uygulama çerçevesi

Userland'ın kalbinde tek bir döngü vardır. Her geçiş, bekleyen her giriş olayını
olay-yokla syscall'ı aracılığıyla boşaltır, onu masaüstüne ya da odaklanmış
uygulamaya yönlendirir ve — bir şey değiştiyse — yeniden çizip sunar. Yalnızca fare
hareket ettiğinde ucuz yolu izler, sadece imlecin yamasını geri yükleyip yeniden
boyar; bir pencere değiştiğinde etkilenen bölgeyi yeniden boyar ve sunar.

Uygulamalar buna küçük bir sözleşme aracılığıyla bağlanır, `app_compiler`'daki `App`
trait'i: bir uygulama bir başlık, verilen bir dikdörtgene çizen bir `draw` rutini ve
gövde-yerel koordinatlarda tıklamalar, sürüklemeler ve tuş basışları alan ve yeniden
çizilmesi gerekip gerekmediğini döndüren bir `on_event` işleyicisi sağlar. Her
yerleşik uygulama tam olarak bu trait'i uygular; pencere yöneticisinin hepsini
tekdüze ele almasını sağlayan şey budur — masaüstü ile içinde çalışan programlar
arasında temiz bir dikiş yeri.