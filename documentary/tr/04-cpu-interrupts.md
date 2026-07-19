# 04 — CPU ve Kesmeler

Bellek çalışır durumdayken çekirdek işlemcinin kendisine yönelir. Modern bir
x86_64 CPU, ayrıcalıklı ve ayrıcalıksız kodu yan yana çalıştırmadan, kesmeleri
iletmeden ve sistem çağrılarını kabul etmeden önce birkaç tablonun doldurulmasına
ihtiyaç duyar. Rusty OS bunların hepsini `arch` altında kurar ve üzerine
zamanlayıcı ile syscall mekanizmasını yerleştirir.

## Tanımlayıcı tabloları

`arch/gdt`'deki Global Descriptor Table, işlemcinin kullandığı segmentleri tanımlar:
çekirdek kod ve verisi, kullanıcı kod ve verisi ve bir Task State Segment. TSS,
daha sonra önem kazanan iki şey taşır. İlki, çift hata (double-fault) işleyicisi
için bir kesme yığını tablosu (IST) girdisidir; böylece feci bir hata bile
üzerinde çalışacak bilinen-iyi bir yığına sahip olur. İkincisi, ayrıcalık yığınıdır
(RSP0); bir ring-3 programı ring 0'a tuzağa düştüğünde işlemcinin geçtiği çekirdek
yığını — zamanlayıcı, CPU'yu farklı bir sürece her verdiğinde bunu günceller,
böylece her biri kendi yığınında çekirdeğe döner.

`arch/idt`'deki Interrupt Descriptor Table, istisna ve kesme işleyicilerini bağlar.
CPU istisnaları — sıfıra bölme, geçersiz opcode, genel koruma hataları, sayfa
hataları, çift hatalar — bir tanılama ekranı çizen bir panik yöneticisine yönlendirilir.
Donanım kesmeleri de bağlanır: zamanlayıcı, klavye ve farenin her biri bir işleyici
alır ve her biri yerel APIC'e kesme-sonu (EOI) sinyali vererek biter. Zamanlayıcı
işleyicisi, sistemin periyodik işinin gerçekleştiği yerdir: bir önyükleme
animasyonunu ilerletir, USB denetleyicisini yoklar ve zamanlayıcıyı çağırır.

## Kesme denetleyicileri ve ACPI

Rusty OS, eski PIC yerine APIC kullanır. `arch/apic`'te eski programlanabilir kesme
denetleyicileri yeniden eşlenir ve ardından tamamen maskelenerek devreden çıkarılır
ve I/O APIC, donanım IRQ hatlarını doğru IDT vektörlerine yönlendirecek şekilde
programlanır. `arch/lapic`'teki yerel APIC etkinleştirilir ve zamanlayıcısı,
zamanlayıcı tıkını tetiklemek üzere periyodik modda yapılandırılır.

Tüm bu donanımı bulmak için çekirdek, önyükleyicinin yakaladığı RSDP işaretçisinden
başlayarak `arch/acpi`'de ACPI tablolarını ayrıştırır. Yerel ve I/O APIC adreslerini
bulmak için kesme modelini okur, işlemcileri sayar ve PCI yapılandırma tabanını
bulur. Ayrıca güç yönetimi yazmaçlarını ve kapatma yolunun ACPI S5 (kapatma)
değerlerini bulmak için ihtiyaç duyduğu DSDT'yi de çıkarır. Tam bir ACPI uygulamasının
sağlayacağı AML yorumlayıcısı mevcut değildir — yöntemleri uygulanmadan bırakılmıştır
— bu yüzden normalde AML üzerinden gidecek saati okumak ya da kapatmak gibi işler,
bunun yerine doğrudan port erişimiyle yapılır.

## Zamanlayıcı

`kernel/schd`'deki zamanlayıcı işbirlikçidir ve bilinçli olarak basittir. Her görev,
içine yapılan ilk bağlam değişikliği önceki bir değişimden dönüş gibi görünecek
şekilde hazırlanmış 64 KB'lik bir yığına sahiptir. Değiştirme, kısa bir assembly
parçasıdır: mevcut görevin yığınına çağrılan tarafından korunan yazmaçları iter,
yığın işaretçisini kaydeder, sonraki görevin yığın işaretçisini yükler, yazmaçlarını
geri çeker ve döner — o görevin en son kaldığı yerin ortasına iner.

## Sistem çağrıları ve ring 3

Çekirdek ile userland arasındaki sınır, `kernel/pscy/syscall`'da kurulan sistem
çağrısıdır. İşlemcinin hızlı SYSCALL/SYSRET komutlarını kullanarak çekirdek bir
giriş noktası ve özel bir çekirdek yığını kaydeder. Userland `syscall` çalıştırdığında,
işlemci çağrı numarası bir yazmaçta ve argümanlar diğerlerinde olacak şekilde
çekirdeğe atlar; küçük bir assembly saplaması bunları C çağrı kuralına yeniden
düzenler ve her çağrıyı uygulayan bir Rust işleyicisine dağıtır — metin yazdırmak,
olayları yoklamak, saati okumak, dosya sistemi işlemleri, kayıt defteri erişimi,
ses çalmak ve daha fazlası.

Ring 3'e ilk baştaki giriş, `kernel/pscy/usermode`'da ele alınır: çekirdek kullanıcı
veri segmentlerini ayarlar, yığın üzerinde kullanıcı kod segmenti, yığın işaretçisi
ve giriş adresiyle bir kesme-dönüş (iretq) çerçevesi oluşturur ve `iretq`'i çalıştırır.
İşlemci ring 3'e iner ve userland'ı çalıştırmaya başlar — ki o andan itibaren sınırın
ötesine yalnızca az önce anlatılan syscall'lar aracılığıyla ulaşabilir.