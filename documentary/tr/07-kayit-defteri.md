# 07 — Kayıt Defteri

Her işletim sisteminin ayarları saklayacak bir yere ihtiyacı vardır — masaüstü
rengi, etkin kullanıcı, ilk kurulumun bitip bitmediği. Rusty OS bunların hepsini
bir kayıt defterinde tutar; ismi ve hiyerarşik anahtar fikrini Windows'tan ödünç
alır ama uygulamayı bilinçli olarak küçük ve okunabilir tutar. `kernel/rgst`
altında bulunur.

## Format ve yapı

Kayıt defteri, düz metin bir anahtar-değer deposudur. Her girdi bir yol ve tipli
bir değerdir; `Sistem/Masaustu/Renk=u32:5249032` gibi bir biçimde satır başına bir
tane yazılır — yol, bir eşittir işareti, tip, iki nokta üst üste ve değer. Üç tip
desteklenir: işaretsiz 32 bit tam sayılar, dizeler ve boolean'lar. İnsan tarafından
okunabilir tutmak bilinçli bir seçimdi; tüm sistem yapılandırması sadece dosya
okunarak incelenebilir ve aynı metin formatı, kayıt düzenleyici uygulamasının
gösterip düzenlediği şeydir.

Bellekte kayıt defteri, bir kilidin arkasında bir girdiler listesidir ve olağan
işlemlere sahiptir — yola göre bir değer al, bir değer ayarla, belirli bir öneke
sahip tüm anahtarları listele. Bunların üzerinde küçük tipli erişimciler bulunur;
böylece çekirdeğin geri kalanı, tipi elle açmadan bir yoldaki bir `u32`'yi bir
varsayılanla isteyebilir. Ayrıca henüz bir kayıt defteri yokken sistemin geri
döndüğü mantıklı bir varsayılanlar kümesi vardır: sistem adı ve sürümü, dil,
masaüstü ve görev çubuğu renkleri, saat dilimi ve başlangıç kullanıcısı.

## Kalıcılık

Tüm depo dört kilobayta sığar ve sistem diskinde tek bir dosyada,
`RSYS/REGISTRY.DAT`'ta tutulur. Yükleme ve kaydetme `kernel/rgst/disk`'te bulunur
ve daha üst düzey dosya API'sinden geçmek yerine doğrudan dosyanın küme zincirini
gezerek, kayıt defteri dosyasını destekleyen ham sektörleri okuyup yazarak çalışır.

Burada gömülü, zorlukla elde edilmiş bir ayrıntı var. Kayıt defterini kaydetmek,
kayıt defteri dosyasının ilk kümesine ihtiyaç duyar ve bunu her seferinde aramak
dizini yeniden taramak demektir. Bundan kaçınmak için ilk küme, kayıt defteri
yüklendiği anda önbelleğe alınır. Yeni kurulmuş bir diskte bu önbellek boş başlar
ve erken bir sürüm, herhangi bir şey onu doldurmadan önce ilk kaydetme
gerçekleştiğinde çökerdi — önbellek soğuksa kümeyi talep üzerine arayarak
düzeltildi. Küçük bir şey, ama tam olarak yalnızca gerçek kurulu bir sistemde
ortaya çıkan ve hızlı bir testte asla görülmeyen türden bir hata.

## Sürücü tablosu ve önbellekler

Kayıt defteri modülü ayrıca bağlanmış sürücülerin tablosuna da sahiptir — her biri
harfi, etiketi, türü, boyutu ve dosya sistemiyle — ve userland'ın sürekli okuduğu
değerler için birkaç atomik önbelleğe; masaüstü ve görev çubuğu renkleri ve geçerli
kullanıcının izin seviyesi gibi. Bunları önbelleğe almak, masaüstünün çizdiği her
tek karede kayıt defteri kilidini almaktan kaçınır.

## Kurtarma

Son parça, `kernel/rgst/recovery`'deki bir kurtarma modudur. Önyüklemede çekirdek,
temel dosyaların mevcut olup olmadığını denetleyebilir — kayıt defterinin kendisi,
userland'ın `CORE.BIN`'i ve beklenen kullanıcı klasörleri — ve kritik bir şey
eksikse, hasarı onaran bir kurtarma moduna girer. Dizin yapısını yeniden
oluşturabilir, kaybolan bir kayıt defterini bellekteki varsayılanlardan yeniden
yazabilir ve `CORE.BIN`'i doğrudan çekirdek imajına gömülü bir kopyadan geri
yükleyebilir. Kısmen hasar görmüş bir kurulumun, basitçe başarısız olmak yerine
kendi kendini iyileştirip yeniden önyükleme yapmasını sağlayan şey budur.