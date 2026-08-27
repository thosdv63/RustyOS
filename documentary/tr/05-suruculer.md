# 05 — Sürücüler

Sürücüler, bir işletim sisteminin fiziksel dünyayla buluştuğu yerdir ve Rusty OS
gerçek donanımla doğrudan konuşur — arada duran bir soyutlama kütüphanesi yoktur.
Bu bölümdeki her şey `drivers` altında bulunur ve hepsi, en başta aygıtları
bulmakla başlar.

## PCI ve düşük seviyeli G/Ç

Çekirdek, `drivers/pci`'de PCI veri yolunu tarayarak donanımı keşfeder. ACPI'nin
sağladığı taban adresine sahip bellek eşlemeli yapılandırma alanını (ECAM)
kullanarak her veri yolunu, aygıtı ve işlevi gezer; her birinin üretici ve aygıt
kimliğini, sınıf ve alt sınıfını ve programlama arayüzünü okur. Bu taramadan, hangi
depolama denetleyicilerinin, ses aygıtlarının ve USB ana denetleyicilerinin mevcut
olduğunu öğrenir. İki küçük yardımcı modülü tamamlar: bir aygıtın taban adres
yazmaçlarını (yazmaçlarının ya da belleğinin bulunduğu BAR'larını) okumak ve bir
aygıtın DMA yapabilmesi için önce ihtiyaç duyduğu bus mastering'i etkinleştirmek.

Altında `drivers/io` bulunur; donanıma erişmenin iki yolunun ince bir sarmalayıcısıdır:
eski aygıtlar için port G/Ç ve modern olanlar için bellek eşlemeli G/Ç. Her MMIO
erişimi, derleyicinin onu asla yeniden sıralamaması ya da elemesi için bir volatile
okuma ya da yazma üzerinden gider ve bir aygıtın zilini çalmadan (doorbell) önce bir
bellek çiti (fence) verilir — donanım, CPU'nun az önce yazdığı belleği okurken
muazzam önem taşıyan bir disiplin.

## Depolama

Rusty OS üç tür disk sürer. `drivers/storage/nvme`'deki NVMe sürücüsü, denetleyiciyi
sıfırlar, DMA belleğinde admin ve G/Ç gönderim ve tamamlama kuyrukları kurar ve
girdiler yazıp ziller çalarak komutlar verir; blok boyutunu ve sayısını öğrenmek
için namespace'i tanımlar, ardından tamamlama kuyruğunu yoklayarak blokları okur ve
yazar. `drivers/storage/ahci`'deki AHCI sürücüsü, SATA disklerini komut listeleri,
FIS yapıları ve fiziksel bölge tanımlayıcı tabloları aracılığıyla ele alır; bağlı
bir sürücüsü olan ilk portu bulur ve READ ile WRITE DMA komutları verir. 
`drivers/storage/ide`'deki IDE ATA sürücüsü, Native PCI veya
Legacy/Compatitable olup olmadığına göre Master ve Slave
destekli şekilde READ ve WRITE komutlarını PIO ile yapar.

Üçüde de aynı küçük arayüzü sunar — okuma, yazma ve blok boyutu işlemleri olan bir
`BlockDevice` trait'i — böylece üstteki dosya sistemi katmanı, hangi tür diskle
konuştuğunu umursamaz.

## USB

USB, en karmaşık sürücüdür. `drivers/usb/xhci`'deki xHCI denetleyici sürücüsü, ana
denetleyicinin halka tabanlı komut ve olay arayüzünü yönetir, aygıt yuvaları ve uç
noktaları ayırır ve bağlı aygıtları sıralar. Gerçek donanımda bu, emülatörlerin
sadece sahip olmadığı tuhaflıkları ele almayı gerektirdi — belirli yapılandırmayı
yalnızca denetleyici durdurulmuşken yazmak ve USB 2 portlarını etkin bir duruma
getirmek için açıkça sıfırlamak.

Ana denetleyicinin üzerinde iki sınıf sürücüsü bulunur. `drivers/usb/hid`'deki HID
sürücüsü, klavye ve fare raporlarını yorumlar; HID kullanım kodlarını karakterlere
ve fare deltalarını imleç hareketine çevirir. `drivers/usb/storage`'deki yığın
depolama sürücüsü, Bulk-Only Transport protokolünü konuşur; USB sürücülerini okuyup
yazmak için SCSI komutlarını komut bloklarına sarar — ki bunlar da dahili disklerle
aynı `BlockDevice` arayüzü üzerinden kendilerini gösterirler.

## Giriş, ses ve gerisi

Eski giriş için bir PS/2 yığını vardır: `drivers/ps2/keyboard`'da tarama kodu
tabloları ve shift/caps işleme özelliği olan bir klavye sürücüsü ve `drivers/ps2/mouse`'da
küçük bir durum makinesi aracılığıyla üç baytlık paketleri bir araya getirip imleci
hareket ettiren bir fare sürücüsü. İkisi de USB HID sürücüsünün kullandığı aynı olay
kuyruğuna beslenir, böylece sistemin geri kalanı bir tuş basışının ya da bir fare
hareketinin nereden geldiğini umursamaz.

Sesin, ortak bir modülün arkasında iki arka ucu vardır: `drivers/audio/hda`'da
denetleyiciyi sıfırlayan, bir DAC ve çıkış pini bulmak için codec'i gezen ve bir
tampon tanımlayıcı listesi aracılığıyla PCM akışı yapan bir Intel HDA sürücüsü ve
yedek olarak `drivers/audio/ac97`'de bir AC'97 sürücüsü. Son olarak `drivers/rtc`,
CMOS portları aracılığıyla gerçek zamanlı saati okur ve bir saat dilimi ofseti
uygular; `drivers/power` ise yeniden başlatma ve ACPI S5 kapatmayı ele alır. Baştan
sona yeni donanım, kesme güdümlü olmaktan çok yoklanır — USB olay halkası ve ses
tıkının ikisi de zamanlayıcıdan servis edilir — bu, sürücü mantığını senkron ve
üzerine akıl yürütmesi daha kolay tutan bilinçli bir seçimdir.
