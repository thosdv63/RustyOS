# 12 — RPE Kurulum

Bir emülatörde çalışan bir işletim sistemi inşa etmek bir şeydir; onu gerçek bir
bilgisayara, Windows ve Linux'un yanına, hiçbir şeyi yok etmeden yerleştirmek başka
bir şeydir. İşte bu, Rusty Preinstallation Environment'ın — RPE'nin — işidir; bir
USB belleği, önyükleyip Rusty OS'u gerçek bir diske dağıtmak için kullanabileceğiniz
bir şeye dönüştüren, kendi kendine yeten bir kurulum ortamı. `kernel/rpe` altında
bulunur.

## Neden başka hiçbir şeye ihtiyacı yok

RPE tasarımının zekice kısmı, kurduğu her şeyi kendi içinde taşımasıdır. Yük —
önyükleyici, çekirdek ve userland'ın `CORE.BIN`'i — derleme zamanında `include_bytes!`
kullanılarak doğrudan RPE çekirdek imajına gömülür. Bu, gerçek donanımda muazzam
önem taşır: kurulum ortamının, USB belleği bir depolama aygıtı olarak okuyabilmeye
bağımlı olmadığı anlamına gelir. Belirli bir makinede USB yığın depolama sıralaması
başarısız olsa bile kurulum yine de çalışır, çünkü dosyalar zaten bellektedir.
Kurulum ortamının yalnızca hedef diske yazabilmesi gerekir, geldiği yerden okuyabilmesi
değil.

## İki geçişli derleme

Yükü gömmek bir yumurta-tavuk sorunu yaratır: çekirdeğin kendisinin bir kopyasını
içermesi gerekir. Makefile bunu iki geçişli bir derlemeyle çözer. İlk geçişte minik
bir saplama (stub) yükle derler ve gerçekte kurulacak olan normal çekirdeği üretir.
O çekirdek sonra hata ayıklama sembollerinden arındırılır — çarpıcı biçimde
küçülterek — ve yük dizinine kopyalanır. İkinci geçişte çekirdek tekrar derlenir, bu
kez o gerçek, arındırılmış çekirdeği yükü olarak gömer ve USB belleğe giden RPE
çekirdeğini üretir.

## Diske güvenle yazmak

`kernel/rpe/install`'daki kurulum ortamı, seçilen bölümü FAT32 olarak biçimlendirir —
geometriyi hesaplar, önyükleme sektörünü ve dosya ayırma tablolarını yerleştirir,
dizin yapısını oluşturur ve yük dosyalarını yerine yazar; hepsini heap kullanmadan
yapar, böylece kısıtlı kurulum ortamında çalışır. Önyükleyiciyi, çekirdeğin iki
kopyasını, `CORE.BIN`'i ve taze bir kayıt defterini yazar.

Kritik olarak, her yazma, dosya sistemi katmanından gelen ve bölüm sınırlarını
dayatan aynı `PartitionDevice` üzerinden gider. Kurulum ortamı, kullanıcının seçtiği
bölümün dışına fiziksel olarak yazamaz, çünkü sınır denetimi aygıtın kendisinde
bulunur. Windows ve Linux'u da barındıran bir diskte çalıştırmayı güvenli kılan
garanti budur.

Diske bir önyükleyici yerleştirmek EFI sistem bölümüne dokunmayı gerektirir ve bu,
`kernel/rpe/esp`'de aşırı bir özenle ele alınır. ESP'yi biçimlendirmez — mevcut FAT'ı
okur, boş kümeler bulur ve yalnızca Rusty'nin önyükleyicisini ekler;
`\EFI\Rusty\BOOTX64.EFI`'yi her zaman yazar ve yedek `\EFI\BOOT\BOOTX64.EFI`'yi ise
yalnızca o yuva boşsa yazar, böylece Windows Önyükleme Yöneticisi'nin asla üzerine
yazmaz. Mevcut dosyalar — Windows, GRUB, orada zaten ne varsa — dokunulmadan bırakılır.

## Kurulum deneyimi

`kernel/rpe/ui`'deki ön yüz, mavi bir gradyanla Windows 7 tarzı bir kurulum
ortamıdır; kurulum sırasında kesmeler devre dışı olduğu için tamamen PS/2 klavyeyle
sürülür. Her diskin GPT'sini okur ve bölümleri sunar; hangilerinin korumalı olduğunu
— kullanıcının seçemeyeceği EFI, Windows ve Linux bölümlerini — ve hangilerinin
seçilebilir boş FAT32 bölümleri olduğunu açıkça işaretler. Bir hoş geldiniz ekranından,
bölüm seçiminden, bir tuş basışını kabul etmeden önce bilinçli bir bir saniyelik
gecikme olan bir son onaydan ve bir kurulum ilerleme kontrol listesinden geçer; bir
geri sayım yeniden başlatmasıyla biter.

Rusty OS'u QEMU'da çalışan bir projeden, gerçek bir dizüstü bilgisayarın FAT32
bölümüne kurulup F12 ile önyüklenen bir projeye dönüştüren parça budur — mevcut
Windows, Linux ve EFI kurulumunu tamamen sağlam bırakarak.