# 10 — Masaüstü Ortamı

Masaüstü, kullanıcının gerçekten gördüğü ve dokunduğu Rusty OS parçasıdır ve
tamamen userland'da, şimdiye kadar anlatılan ilkel öğelerden inşa edilmiştir.
Userland'ın `ui` modülü altında bulunur ve Windows 7 masaüstünün sadık, elle
çizilmiş bir yorumudur — sürükleyip yeniden boyutlandırabileceğiniz pencereler,
yuvarlak bir başlat düğmesi olan bir görev çubuğu, iki panelli bir başlat menüsü ve
masaüstünde seçip taşıyıp yeniden adlandırabileceğiniz simgeler.

## Pencereler ve pencere yöneticisi

`ui/window`'da tanımlanan bir pencere, bir başlığı, bir gövdesi, bir durumu (normal,
büyütülmüş ya da küçültülmüş) ve olağan başlık çubuğu kontrolleri olan bir
dikdörtgendir. Kendini nasıl çizeceğini bilir — parlak başlık çubuğu, yuvarlak
çerçeve, küçült, büyüt ve kapat düğmeleri — ve bir tıklamanın hangi bölgeye
düştüğünü nasıl anlayacağını.

`ui/window_mgr`'daki pencere yöneticisi, açık pencerelerin listesine sahiptir ve o
listeyi z-sırası olarak ele alır: son pencere en öndekidir. Bir pencereyi başlık
çubuğundan sürüklemeyi, ekran dışında ya da görev çubuğunun arkasında kaybolamaması
için sınırlandırarak ele alır; büyütme ve geri yüklemeyi, önceki konumu hatırlayarak
ele alır; ve tıklanan bir pencereyi öne getirir. Ayrıca açık pencereler için görev
çubuğu düğmelerini çizer ve onlara yapılan tıklamaları doğru pencereyi geri yükleme
ya da odaklamaya geri eşler.

## Görev çubuğu ve başlat menüsü

`ui/taskbar`'daki görev çubuğu, altta parlak bir kenarı olan koyu çubuktur; elle
çizilmiş bir "R" olan yuvarlak parlak bir başlat küresi ve saat ile tarih için
gerçek zamanlı saati okuyan bir saat içerir. Küreye tıklamak başlat menüsünü açıp
kapatır.

`ui/taskbar_manager`'daki o menü, iki panelli Windows 7 düzenidir: solda beyaz bir
uygulamalar paneli — her biri kendi küçük elle çizilmiş simgesiyle — ve sağda
kullanıcının avatarı, klasörlerine kısayollar ve kapatma ile yeniden başlatma
düğmeleri olan yarı saydam bir panel. Menüden bir uygulama seçmek, sistemden onu
başlatmasını ister; ana döngü bunu alır ve yeni bir pencereye dönüştürür.

## Masaüstü yüzeyi

`ui/desktop`'taki masaüstü arka planının kendisi, büyük parlak bir "R" logosu ve
Rusty OS adıyla gradyan duvar kâğıdıdır. Üzerine katmanlanmış `ui/desktop_manager`,
etkileşimli simgeleri ele alır — Bilgisayar, Geri Dönüşüm Kutusu ve kullanıcının
masaüstünde bulunan dosya ve klasörler. Bir simge seçmeyi, bir grubu lastik bant
dikdörtgeniyle sürükleyerek seçmeyi, simgeleri yeni konumlara sürüklemeyi, açmak
için çift tıklamayı, öğe oluşturmak ve silmek için bir sağ tık bağlam menüsünü ve
satır içi yeniden adlandırmayı destekler. Bir klasör ya da dosya açmak, dosya
gezginini ya da doğru uygulamayı başlatmak için sistem üzerinden bir istek yönlendirir.

## Hepsini bir araya getirmek

Pencereler ile içlerindeki programlar arasındaki tutkal, `ui/app_mgr`'daki uygulama
yöneticisidir. Bir şey bir uygulamayı başlatmak istediğinde, doğru olanı oluşturur,
onun için bir pencere açar ve eşleşmeyi hatırlar. Her karede her uygulamayı kendi
penceresinin gövdesine çizer ve tıklamaları, sürüklemeleri ve tuş basışlarını
odaklanmış pencereye sahip olan uygulamaya yönlendirir — ekran koordinatlarını
uygulamanın beklediği gövde-yerel koordinatlara çevirerek. Ayrıca pencereler
kapandığında temizlik yapar ve bir uygulamanın başka birini başlatma isteklerini
ele alır; dosya gezgininin bir metin dosyasını editörde açması gibi.

İki ekran tüm deneyimi çerçeveler. Masaüstü hiç görünmeden önce, `ui/oobe`'deki OOBE
sihirbazı ilk kez kullanan birini bir ad, isteğe bağlı bir şifre ve bir masaüstü
rengi seçmekten geçirir, sonra bunları kayıt defterine yazar ve kullanıcının
klasörlerini oluşturur. Ondan sonra, `ui/login`'deki giriş ekranı kullanıcının
avatarını sunar ve bir şifre ayarlanmışsa, masaüstüne geçmesine izin vermeden önce
onu ister. Paylaşılan bir tema modülü her rengi tek bir yerde tutar, böylece tüm
ortam görsel olarak tutarlı kalır.