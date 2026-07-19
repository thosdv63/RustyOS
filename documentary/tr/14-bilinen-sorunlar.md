# 14 — Bilinen Sorunlar

Bir sistemin her dürüst anlatımı, henüz çalışmayan kısımları da içerir ve Rusty OS
bir istisna değil. Bu bölüm, pürüzlü kenarların, ertelenmiş işlerin ve yalnızca
henüz hiçbir şey onları zorlamadığı için geçerli olan varsayımların bilinçli olarak
açık sözlü bir listesidir. Bunların hiçbiri sistemin önyükleme yapmasını, kurulmasını
ve çalışmasını durdurmaz — ama dikişlerin nerede olduğunu bilmek, kod tabanını
anlamayı ve iyileştirmeyi kolaylaştırır.

## AML yorumlayıcısı olmadan ACPI

ACPI uygulaması statik tabloları okur — kesme modeli, güç yazmaçları, DSDT — ama bir
AML yorumlayıcısı içermez. Tam bir ACPI yığınının değerlendireceği yöntemler
uygulanmadan bırakılmıştır. Pratikte bu, normalde AML aracılığıyla yapılacak
herhangi bir şeyin, örneğin saati okumanın ya da makineyi kapatmanın, bunun yerine
doğrudan donanım port erişimiyle ele alındığı anlamına gelir. Bu, test edilen
donanımda çalışır, ama gerçek bir AML yorumlayıcısının olacağı kadar genel değildir
ve olağandışı firmware'e sahip bir makinenin farklı davranabileceği bir alandır.

## Ölçek varsayımları

Sistemin çeşitli kısımları, şeylerin tek bir örneğini varsayar. Depolama sürücüleri,
hepsini sıralamak yerine tek bir namespace'i ya da tek bir bağlı sürücüyü hedefler.
Kayıt defteri dört kilobaytla sınırlıdır; bu, bugün tuttuğu ayarlar için cömerttir
ama büyüyen bir depo yerine sabit bir tavandır. Bu sınırlar mevcut sistem için
sorunsuzdur ve çalışan bir bütüne ulaşmak için doğru sadeleştirmelerdi, ama daha
ayrıntılı kurulumları desteklemek için büyümesi gereken ilk şeylerdir.

## Kullanılmayan süreç yapısı

Kod tabanı, tam yazmaç durumu ve süreç yönetimi alanlarıyla birlikte, şu anda
kullanılmayan bir süreç yapısı içerir; çünkü Rusty OS, diskten rastgele programlar
yüklemek yerine tek bir userland ikilisi çalıştırır. ELF yürütülebilir dosyalarını
çalışma zamanında yüklemek — gerçek bir çok süreçli model — doğal bir sonraki
adımdır ve bunun iskelesi kısmen yerindedir, ama henüz bağlanmamıştır.

## Yol haritası

Yukarıdaki sorunları düzeltmenin ötesinde, net bir sonraki yön ağ oluşturmadır: ağ
sürücülerini ve internet adreslerinden veri çekme yeteneğini uygulamak, ki bu, bütün
bir yeni uygulama kategorisini açardı. Bir çalışma zamanı ELF yükleyicisi, tek
userlandlı modeli gerçek bir çok süreçli sisteme dönüştürürdü. Ve çekirdek, üzerine
tamamen farklı bir userland yazılabilecek şekilde bilinçli olarak yapılandırılmıştır
— syscall sınırı, masaüstünün altındaki çekirdeğin yalnızca olası bir istemcisi
olacak kadar temizdir.

Bunların herhangi birine yönelik katkılar ve öneriler memnuniyetle karşılanır. Her
şeyi sıfırdan yazmanın amacı her katmanı anlamaktı; belgelemenin amacı ise başka
birinin de anlayabilmesidir.