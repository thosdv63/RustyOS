# 02 — Açılış Süreci

Bir UEFI makinesi açıldığında, firmware önyüklenebilir bir yürütülebilir dosya
arar ve onu, zengin bir servis kümesi hâlâ kullanılabilir durumdayken başlatır:
bellek ayırabilir, dosya okuyabilir, grafik donanımını sorgulayabilir ve aygıtları
sıralayabilir. Rusty OS'un önyükleyicisi `boot` crate'inde bulunur ve bu pencereden
tam anlamıyla yararlanır. Görevi, çekirdeği bulup yüklemek, makineyi ona
tanımlamak ve sonra yoldan çekilmektir.

## Önyükleme yöneticisi

Herhangi bir şey yüklenmeden önce önyükleyici bir önyükleme yöneticisi sunar —
Windows 7 önyükleme ekranına, vurgulanan seçim çubuğuna ve geri sayım sayacına
kadar bilinçli olarak benzetilmiş, metin modunda bir menü. Bu, süslemeden fazlasıdır.
Önyükleyici, firmware'in açığa çıkardığı her dosya sistemini tarar ve önyüklenebilir
hedefler arar: bir Rusty çekirdeği (ya önyükleme biriminin kökünde `kernel.elf`
olarak ya da kurulu bir diskte `RSYS\KERNEL.ELF` olarak) ve tanıyabileceği diğer
işletim sistemleri — kendi önyükleme yöneticisi üzerinden Windows, GRUB üzerinden
Ubuntu ya da standart yedek yolundaki herhangi bir genel UEFI yükleyici.

Bunların her biri menüde bir girdi olur. Yalnızca bir Rusty çekirdeği mevcutsa ve
hiçbir şey başarısız olmadıysa, önyükleyici onu hemen başlatabilir; aksi hâlde,
herhangi bir tuşun iptal ettiği on saniyelik bir geri sayımla birlikte listeyi
gösterir. Menü ayrıca küçük bir araç taşır — RAM'i bir megabaytlık parçalar hâlinde
ayırıp desen testinden geçiren ve arızalı blokları raporlayan bir bellek tanılama
aracı. Rusty yerine başka bir işletim sistemi seçmek bir chainload tetikler:
önyükleyici o işletim sisteminin `.efi` dosyasını belleğe okur ve başlatır,
makineyi tamamen ona devreder.

## Çekirdeği yüklemek

Bir Rusty çekirdeği seçildiğinde, önyükleyici onu bir ELF dosyası olarak ayrıştırır.
Başlığı doğrular, program başlıklarını gezer ve her yüklenebilir segment için,
segmentin fiziksel adresinde sayfalar ayırıp içeriği kopyalar; segmentin dosyada
saklanandan fazla gerektirdiği sondaki boşluğu sıfırlar. Son atlama için giriş
noktası adresi hatırlanır.

Çekirdek bellekteyken önyükleyici `BootInfo` yapısını bir araya getirir.
Framebuffer'ın taban adresini, genişliğini, yüksekliğini ve stride'ını almak için
Graphics Output Protocol'ü sorgular. Bellek haritasını okur ve hangi bölgelerin
kullanılabilir RAM olduğunu kaydeder. Çekirdeğin daha sonra kesme denetleyicilerini
ve güç yönetimi donanımını keşfetmek için ihtiyaç duyacağı RSDP işaretçisini bulmak
üzere ACPI 2.0 yapılandırma tablosuna bakar. Ve önyükleme biriminin bir `RPE.FLAG`
dosyası taşıyıp taşımadığını denetler — çekirdeğe, kurulu sistemi başlatmak yerine
kurulum ortamı olarak çalışması gerektiğini söyleyen işaret.

## Dönüşü olmayan nokta

Son adımlar en hassas olanlardır. Önyükleyici, bellek bölgesi dizisi ve `BootInfo`'nun
kendisi için sayfalar ayırır, ardından `exit_boot_services`'i çağırır. Bu tek yönlü
bir kapıdır: ondan sonra firmware'in servisleri kaybolur ve bellek haritası dondurulur.
Önyükleyici, firmware'in döndürdüğü son haritayı alır, kullanılabilir bölgeleri yazar,
tamamlanmış `BootInfo`'yu doldurur ve ardından devir teslimi gerçekleştirir —
`BootInfo` işaretçisini bir yazmaca yerleştirip çekirdeğin giriş noktasına atlayan
kısa bir assembly parçası. Önyükleyici asla geri dönmez; buradan sonra kontrol
çekirdektedir.

```
  diskleri tara ─► önyükleme yöneticisini göster ─► çekirdek ELF'ini yükle
                                                         │
                                                         ▼
              GOP + bellek haritası + ACPI + RPE bayrağını sorgula
                                                         │
                                                         ▼
              exit_boot_services ─► çekirdek girişine atla
```

Burada gösterilen özen — ELF'i doğrulamak, doğru sayfaları rezerve etmek, firmware
görevden alınmadan önce ACPI işaretçisini yakalamak — çekirdeğin diğer tarafta
bilinen, iyi tanımlanmış bir durumdan başlamasını sağlayan şeydir.