# 08 — Grafik

Rusty OS'ta görünen her şey teker teker piksel olarak çizilir. GPU hızlandırması ya
da bir pencereleme kütüphanesi yoktur — yalnızca firmware'in devrettiği doğrusal
bir framebuffer ve içine renk yazan kod. İlginç mühendislik, bunu tam bir Aero
tarzı masaüstünün duyarlı hissettirecek kadar hızlı ve temiz yapmakta yatar.

## Framebuffer ve çift tamponlama

Önyükleyici, framebuffer'ın adresini, boyutlarını ve stride'ını yakaladı ve
çekirdeğe iletti; çekirdek de bunları userland'a aktarır. Ancak doğrudan o
framebuffer'a çizmek, yarı çizilmiş bir kare ekranda gösterileceği için görünür
yırtılma ve titremeye yol açardı. Rusty OS bunu standart yolla, çift tamponlamayla
çözer. Her şey sıradan bellekteki bir arka tampona çizilir ve yalnızca bir kare
tamamlandığında gerçek framebuffer'a kopyalanır — buna sunma (presenting) denir.

Burada iki adres tekrar eder: `0x10000000`'deki arka tampon ve `0x80000000`'deki
fiziksel framebuffer. Renderer ilkine çizer ve ikincisine sunar. Bir bütün kareyi
sunmak tek bir hızlı bellek kopyasıdır, ama renderer yalnızca bir dikdörtgeni de
sunabilir — yalnızca değişen bir bölgeyi kopyalayarak — ki bu, imleç hareketini ve
küçük güncellemeleri her seferinde tüm ekranı yeniden boyamak yerine ucuz kılan
şeydir.

## Renderer

Aslında iki renderer vardır — biri çekirdekte erken önyükleme ve panik ekranları
için, diğeri userland'da masaüstü için daha zengin olanı — ama aynı temeli
paylaşırlar. En altta, ekrana karşı sınır denetimi yapan ve userland durumunda rengi
framebuffer'ın bayt sırasına çeviren tek piksellik bir yazma bulunur. Bunun üzerinde,
diğer her şeyin üzerine inşa edildiği ilkel öğeler vardır: dolu dikdörtgenler,
gradyanlar, çizgiler ve daireler.

Userland renderer'ı daha da ileri gider, masaüstüne Windows 7 görünümünü veren
alana. Dikey gradyanlara, üst kenar boyunca bir vurguyla açıktan koyuya solan
parlak (glossy) dolgulara, yuvarlak köşeli dikdörtgenlere ve yeni bir rengi tamponda
zaten olan neyse onunla karıştıran alfa harmanlamaya sahiptir. Bunlar birlikte, Aero
estetiğini tanımlayan cam benzeri düğmeleri, yuvarlak pencere çerçevelerini ve yarı
saydam seçim dikdörtgenlerini üretir — hepsi elle, piksel piksel hesaplanır.

## Metin

Metin, bir 8×8 bitmap yazı tipinden çizilir. Her karakter sekiz bayttır, satır
başına bir tane; her bit bir pikselin açık olup olmadığına karar verir. Renderer bir
karakteri, o bitleri gezip pikselleri doldurarak çizer; isteğe bağlı olarak tam
sayı bir çarpanla büyütülerek aynı yazı tipi hem küçük etiketlere hem de büyük
başlıklara hizmet eder. Çekirdek ve userland biraz farklı yazı tipi tabloları taşır
— çekirdeğinki aldığı tarama kodu kümesine göre, userland'ınki düz ASCII'ye göre
düzenlenmiştir — ama çizim yaklaşımı aynıdır.

## İmleç

Fare imleci, sürekli hareket eden tek şeydir ve onu takip etmek için tüm ekranı
yeniden çizmek israf olurdu. Bunun yerine renderer, çizmeden önce imlecin altındaki
küçük piksel yamasını kaydeder — 12×19'luk bir bölge — ve imleç hareket ettiğinde,
yeni konumda kaydedip çizmeden önce o yamayı geri yükler. Bu kaydet-geri yükle
hilesi, fareyi hareket ettirmenin maliyetinin ekranın boyutuyla değil imlecin
boyutuyla orantılı olması demektir ve her piksel yazılımla yönetiliyor olsa bile
işaretçinin akıcı bir şekilde kaymasının nedeni budur.

```
   arka tampona çiz  ─►  sun (framebuffer'a kopyala)
        ▲                      │
        │  imleç altını kaydet │  yalnızca değişen dikdörtgeni sun
        └──────────────────────┘
```