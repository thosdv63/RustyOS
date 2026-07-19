# 13 — Derleme ve Çalıştırma

Rusty OS, birkaç ayrı derlemeyi düzenleyen ve onları önyüklenebilir disk imajlarına
dönüştüren bir Makefile ile inşa edilir. Bu bölüm, neye ihtiyacınız olduğunu,
derlemenin gerçekte ne yaptığını ve sistemi nasıl çalıştıracağınızı kapsar — ister
bir emülatörde, ister gerçek donanımda.

## Ön koşullar

Rust nightly araç zinciri kurulu bir Linux ana makinesine ihtiyacınız var; çünkü
çekirdek kararsız özelliklere dayanır ve bare-metal hedefler için derlenir. İki hedef
kullanılır: çekirdek ve userland için `x86_64-unknown-none` ve önyükleyici için
`x86_64-unknown-uefi`. Rust'ın ötesinde derleme, QEMU'ya (sürüm 10.1 ya da daha
yenisi), OVMF UEFI firmware imajlarına ve FAT dosya sistemlerini doldurmak için
`mtools`'a ihtiyaç duyar. GPT ve kurulum ortamı hedefleri için `sgdisk` de gereklidir.

## Derleme ne yapar

Derlemenin, parçaların birbirine nasıl bağımlı olduğu nedeniyle belirli bir sırası
vardır. Userland önce derlenir ve düz bir ikiliye, `core.bin`'e dönüştürülür. Sonra
çekirdek derlenir — ve çekirdek `core.bin`'i gömdüğü için, her zaman taze bir kopya
alsın diye derleme önbelleği önce temizlenir. Önyükleyici, ayrı bir UEFI uygulaması
olarak derlenir. Son olarak imajlar bir araya getirilir: önyükleyiciyi, çekirdeği ve
userland'ı taşıyan bir önyükleme imajı ve ayrı sanal diskler — bir NVMe diski ve bir
SATA diski, her biri FAT32 biçimlendirilmiş ve bir sistem dizini ile varsayılan bir
kayıt defteriyle tohumlanmış, artı test için bir USB diski.

## QEMU'da çalıştırmak

Ana hedef her şeyi derler ve QEMU'yu tam bir sanal donanım takımıyla başlatır:

```bash
make run
```

QEMU çağrısı bilinçli olarak zengindir, çünkü her sürücüyü çalıştırır: bir NVMe
denetleyicisi, bir AHCI SATA denetleyicisi, klavye, fare ve depolama aygıtı olan bir
xHCI USB denetleyicisi ve hem Intel HDA hem AC'97 sesi ekler. Aynı derlemenin, tüm
sürücülerine karşı aynı anda test edilmesini sağlayan şey budur. İki ayrıntı
bilinmeye değer. KVM hızlandırması esastır — saf yazılım emülatörü kullanılabilir
olamayacak kadar yavaştır — ve bir IOMMU sürücüsü mevcut olmadan bir IOMMU aygıtı
eklenmemelidir, çünkü NVMe DMA'yı bozar.

## Kurulum ortamı ve kurulu sistem hedefleri

Üç hedef daha kurulum iş akışını kapsar. `make run-rpe`, RPE kurulum USB imajını (iki
geçişli derleme aracılığıyla) ve gerçek bir makine gibi düzenlenmiş bir sanal GPT
diski oluşturur — bir EFI bölümü, bir Microsoft ayrılmış bölümü, Windows benzeri bir
NTFS bölümü, bir Linux bölümü ve boş bir hedef bölüm — sonra o diske kurabilesiniz
diye USB'yi önce önyükler. `make run-installed`, ortaya çıkan diski doğrudan önyükler;
makine zaten kurulmuş bir sistemi başlatıyormuş gibi. Ve `make gpt-disk`, yalnızca
bölümlenmiş test diskini hazırlar.

## Derleme profili üzerine bir not

Apaçık olmayan bir derleme ayarı vurgulamaya değer, çünkü zorlukla öğrenildi.
Geliştirme profili, taşma denetimlerini ve hata ayıklama savlarını devre dışı bırakır.
Bu performansla ilgili değil — bir doğruluk gereksinimi. Rust nightly'nin işaretçi
okuma ön koşul denetimleri, bir bare-metal çekirdeğin işleyicisi olmadığı tuzak
komutları yayar ve bunları etkin bırakmak, çekirdeğin aslında sorunsuz olan işlemlerde
çökmesine neden olur. Profilde bunları kapatmak, çekirdeğin hiç çalışabilmesini
sağlayan şeydir.

## Gerçek donanımda çalıştırmak

Gerçek donanıma giden yol RPE'den geçer. RPE USB imajını gerçek bir USB belleğe
yazarsınız, hedef makineyi firmware'in önyükleme menüsü aracılığıyla (tipik olarak
F12) ondan önyüklersiniz ve kurulum ortamını kullanarak Rusty OS'u boş bir FAT32
bölümüne yazarsınız. Kurulum ortamı yalnızca seçtiğiniz bölüme dokunduğu ve
önyükleyicisini ESP'ye orada olanı bozmadan eklediği için, bu, zaten Windows ya da
Linux çalıştıran bir makinede yapılabilir — ki Rusty OS'un gerçek bir dizüstü
bilgisayarda çalışır hâle gelmesi tam olarak böyle oldu.