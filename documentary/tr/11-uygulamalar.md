# 11 — Uygulamalar

Bir işletim sistemi, ancak üzerinde gerçekten yapabildiğiniz şeyler kadar
inandırıcıdır ve Rusty OS tam bir yerleşik uygulama kümesiyle gelir. Hepsi
userland'ın `apps` modülü altında bulunur ve her biri aynı `App` trait'ini uygular
— bir başlık, bir çizim rutini, bir olay işleyicisi — böylece pencere yöneticisi
onları tekdüze ele alır. Değişen şey, her birinin kendine sunulan syscall'larla ne
yaptığıdır.

## Dosya Gezgini

`apps/explorer`'daki dosya gezgini, kümenin en özellik açısından tam olanıdır. Bir
dizinin içeriğini dizin syscall'ı aracılığıyla listeler; sürücüleri, klasörleri ve
dosyaları türü ve boyutuyla gösterir. Geri ve ileri çalışsın diye gezinme geçmişi,
bir adres içerik yolu (breadcrumb) ve bir kısayollar kenar çubuğu tutar. Açmak için
çift tıklamayı destekler — klasörler içeri girer ve dosyalar doğru uygulamayı
başlatır; bir `.BMP`'yi resim görüntüleyiciye, bir `.RAW`'ı ses sürücüsüne ve
metin benzeri dosyaları editöre gönderir. Kes, yapıştır, sil ve yeniden adlandır ile
bir sağ tık bağlam menüsüne sahiptir ve — önemlisi — kritik sistem dosyalarını korur;
kayıt defterini, çekirdeği, `CORE.BIN`'i ya da sistem dizinlerini silmeyi reddeder ve
riskli her şey için bir onay iletişim kutusu gösterir.

## Düzenleme ve çizim

`apps/notepad`'deki Not Defteri, gerçek bir metin editörüdür: dosyaları yükler ve
kaydeder, satırları ve bir imleci izler, her iki yönde kaydırır ve kaydet, farklı
kaydet ve yeni işlevlerini sunar; kaydedilmemiş değişiklikler olduğunda bir onay
istemiyle. `apps/paint`'teki Paint, bir renk paleti, ayarlanabilir fırça boyutları,
bir silgi ve resimleri açıp kaydetme yeteneği olan küçük bir çizim programıdır —
çizgileri bir Bresenham çizgisiyle çizer, böylece sürüklemek sürekli bir iz üretir ve
gerçek BMP dosyalarını okuyup yazar.

## Yardımcı araçlar

`apps/hesap`'teki hesap makinesi, olağan işlemler ve klavye desteğiyle çalışan bir
kayan noktalı hesap makinesidir. `apps/regedit`'teki kayıt düzenleyici, her kayıt
anahtarını listeler ve değerleri yerinde düzenlemenize ya da yenilerini eklemenize
izin verir; bir değerin bildirilen tipiyle eşleştiğini doğrular. `apps/gorevmgr`'daki
görev yöneticisi, sistem bilgisi syscall'ından canlı CPU ve RAM kullanımını açık
pencerelerin listesiyle birlikte gösterir ve seçilen birini sonlandırabilir.
`apps/ayarlar`'daki ayarlar paneli, kategorize edilmiş bir görünüm sunar — görünüm,
sistem, ses, güç — burada masaüstü rengini değiştirebilir, kullanıcıyı yeniden
adlandırabilir, sesi test edebilir ve kapatabilir ya da yeniden başlatabilirsiniz.

## Komut İstemi

`apps/cmd`'deki komut istemi, yaklaşık yirmi beş komutlu gerçek bir kabuktur. Göreceli
ve mutlak yolları çözer, sürücüleri Windows'un yaptığı gibi değiştirir ve tanıdık
kümeyi uygular: `dir`, `cd`, `cls`, `echo`, `type`, `copy`, `del`, `ren`, `mkdir`,
`move`, `date`, `time`, `ver` ve `color`, artı Rusty'ye özgü olanlar — kayıt defterini
sorgulamak için `reg`, pencereleri yönetmek için `tasklist` ve `taskkill`, uygulama
başlatmak için `start` ve `shutdown`. `color` komutu için klasik on altı renkli
paleti bile taşır.

## Görüntüleme ve destekleyici kod

`apps/resim`'deki resim görüntüleyici, pencereye sığdır ya da bire bir geçişiyle BMP
resimlerini gösterir ve `apps/hakkinda`'daki hakkında penceresi, sürümü gösterir ve
— kayıt defterinin canlı olduğunun küçük bir kanıtı olarak — geçerli masaüstü rengini
ondan geri okur. Bunların birçoğunun altında, 24 ve 32 bit sıkıştırılmamış bitmap'leri
çözen ve kodlayan, `apps/bmp`'deki paylaşılan bir BMP codec'i bulunur; Paint'in, resim
görüntüleyicinin ve gezginin hepsinin aynı resim formatını konuşmasını sağlayan şey
budur.

Bir arada ele alındığında bu uygulamalar, altlarındaki çekirdeği ve masaüstünü gerçek,
kullanılabilir bir bilgisayar gibi hissettiren bir şeye dönüştürür — dosyalara göz
atabilir, yazıp çizebilir, aritmetik yapabilir, sistemi inceleyebilir ve bir komut
satırına inebilirsiniz; hepsini de Rusty OS'tan hiç ayrılmadan.