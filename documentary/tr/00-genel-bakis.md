# 00 — Genel Bakış

Rusty OS, 64 bit Intel ve AMD makineleri için, tamamen Rust ile ve altında hiçbir
işletim sistemi olmadan yazılmış bir masaüstü işletim sistemidir. Zor işleri yapan
bir Linux çekirdeği, bir C çalışma zamanı ya da `std` kütüphanesi yoktur —
firmware kontrolü devrettiği andan itibaren olan biten her şey, bu depodaki
koddur. Önyükleme yapar, belleği yönetir, disklerle ve USB aygıtlarıyla konuşur,
pencereli bir masaüstü çizer ve gerçek uygulamalar çalıştırır; bunların hepsini de
bare-metal üzerinde yapar.

Proje, Windows 7'den ilham alır. Bu ilham bilinçlidir ve her yerde kendini
gösterir: yuvarlak bir başlat düğmesi olan parlak bir görev çubuğu,
küçült/büyüt/kapat kontrolleri olan sürüklenebilir pencereler, iki panelli bir
başlat menüsü, bir ilk kurulum sihirbazı ve bir giriş ekranı. Palet ise Windows
mavisi değil — ismin oynadığı "Rusty" (paslı) kimliği yansıtan, sıcak bir turuncu
ve amberdir. Amaç asla Windows'u piksel piksel klonlamak değildi; daha basit bir
soruyu yanıtlamaktı: bu deneyimin tamamını, işlemcinin çalıştırdığı ilk komuttan
başlayarak kendin inşa etmek nasıl bir his olurdu?

## Neden var

Bu işletim sistemi, arkasında uzun bir geçmişi olan kişisel bir projedir. Bilgisayarların yüzeyin altında gerçekte nasıl çalıştığına dair ilgiyi ilk uyandıran makineyle başladı.
Rusty OS'tan önce daha eski denemeler vardı; bunların arasında, çalışan bir AHCI disk sürücüsüne kadar ilerleyen, C ile yazılmış bir sürüm de bulunuyordu. RustyOS, sonunda bitiş çizgisine ulaşan yeniden yazımdır: gerçek bir bilgisayara kurulup kullanılacak kadar tamamlanmış bir sistem.

Baştan sona benimsenen felsefe, her katmanı içe aktarmak yerine anlamaktır. Çoğu
projenin bir dosya sistemini ayrıştırmak ya da bir donanım parçasını sürmek için
bir crate ekleyeceği yerde, Rusty OS bunu doğrudan uygular; çünkü bu işin bütün
amacı, uygulamanın kendisidir. Sonuç, tek bir tuş basışını klavye denetleyicisinin
bir kesme tetiklediği andan itibaren çekirdeğin işleyicisine, oradan bir olay
kuyruğuna, syscall sınırının ötesine ve nihayet ekranda yeniden çizilen bir metin
kutusuna kadar izleyebileceğiniz bir kod tabanıdır.

## Neler yapabilir

Birinci sürümde Rusty OS, kendi UEFI önyükleyicisiyle başlar, işlemcinin tanımlayıcı
tablolarını ve kesme denetleyicilerini ilklendirir, sayfalama ve bir heap kurar ve
donanımını bulmak için PCI veri yolunu tarar. NVMe ve AHCI depolamayı, bir xHCI
denetleyicisi üzerinden USB aygıtlarını, PS/2 klavye ve fareleri ve Intel HDA ya
da AC'97 sesi sürer. Küçük bir sanal dosya sistemi katmanı üzerinden bir FAT32
dosya sistemini okur ve yazar, ayarlarını düz metin bir kayıt defterinde tutar ve
çekirdekle derli toplu bir sistem çağrısı arayüzü üzerinden konuşan bir userland'ı
ring 3'te çalıştırır.

Bu çekirdeğin üzerinde tam bir masaüstü ortamı bulunur — bir pencere yöneticisi,
bir görev çubuğu, bir başlat menüsü, sürükleyip yeniden adlandırabileceğiniz
masaüstü simgeleri — ve bir dizi yerleşik uygulama: bir dosya gezgini, bir metin
editörü, bir çizim programı, bir hesap makinesi, bir kayıt düzenleyici, bir görev
yöneticisi, bir ayarlar paneli, bir komut istemi ve bir resim görüntüleyici.

Son olarak Rusty OS, kendi kurulum ortamı olan Rusty Preinstallation Environment
ile gelir; bu ortam, sistemi mevcut bir Windows ya da Linux kurulumunun yanına,
onlara zarar vermeden gerçek bir GPT diske yazabilir.

## Bu dokümantasyon nasıl okunur

İzleyen bölümler, yığının en altından en üstüne doğru ilerler. Bir sonraki bölüm
genel mimariyi ve kod tabanının nasıl düzenlendiğini ortaya koyar; ondan sonra her
alt sistem, kabaca makinenin kendisinin onları hayata geçirdiği sırayla kendi
bölümüne sahip olur — önyükleme, bellek, işlemci, sürücüler, dosya sistemi, kayıt
defteri, grafik, userland, masaüstü, uygulamalar ve kurulum ortamı. Son bölümler,
her şeyin nasıl derlenip çalıştırılacağını anlatır ve hâlâ pürüzlü olan kısımların
dürüst bir dökümünü verir.