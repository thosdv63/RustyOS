# 03 — Bellek

Çekirdek kontrolü ele aldığında, bellek çalışmadan neredeyse başka hiçbir şey
olamaz. Her ayırma, her sürücü tamponu, ekrandaki her pencere nihayetinde bellek
alt sisteminden gelir ve Rusty OS bunu üç katmanda inşa eder: gerçek RAM'in 4 KB'lik
sayfalarını dağıtan bir fiziksel çerçeve ayırıcısı, sanal adresleri bu çerçevelere
eşleyen bir sayfa tablosu yöneticisi ve eşlenmiş bir bölgeden küçük nesneler yontan
bir heap ayırıcısı.

## Fiziksel çerçeve ayırıcısı

En alt katman, `mm/pfa`'da, fiziksel belleği bir bitmap ile izler — her 4 KB'lik
çerçeve için bir bit; çerçeve kullanımdayken set, boşken temiz. Başlangıçta
önyükleyicinin ilettiği bellek haritasını okur, en yüksek kullanılabilir adresi
bulur ve bitmap'i dört gigabaytlık bir tavana kadar tüm RAM'i kapsayacak şekilde
boyutlandırır. Bitmap'in kendisi bir yerde bulunmak zorundadır, bu yüzden ayırıcı
onu bulabildiği en büyük kullanılabilir bölgeye yerleştirir, ardından her şeyi
kullanımda olarak işaretler ve yalnızca firmware'in sıradan bellek olarak
bildirdiği bölgeleri serbest bırakır.

Bu ayırıcıyı ilginç kılan, neyi dağıtmayı reddettiğidir. Birkaç bölge elle rezerve
edilmiştir. İlk beş megabayt — userland ikilisinin ve yığınının bulunduğu yer —
kalıcı olarak kullanımda işaretlenir ve ayırıcının normal dağıtım rutini tamamen
onların üzerinden başlar. Ayrıca ses için ayrılmış bir megabaytlık bir yedek bölge
vardır. Sebep incedir ama önemlidir: DMA yeteneği olan donanım doğrudan fiziksel
belleğe yazar ve bir disk ya da ses aktarımına çalışan userland'ın belleğindeki bir
çerçeve verilseydi, canlı bir programı sessizce bozardı. Bu bölgeleri ayırıcının
erişiminden çıkararak, ayıklanması imkânsız bir çökme kategorisi tasarım yoluyla
ortadan kaldırılır.

## Sayfa tablosu yöneticisi

Çerçeve ayırıcısının üzerinde `mm/ptm`, sanal bellekle ilgilenir. Rusty OS,
firmware'in kurduğu birebir eşlemeye (identity mapping) dayanır — sanal adresler
fiziksel adreslere eşittir — ve gerektiğinde onu genişletir. Yönetici, tek bir
sayfayı bir çerçeveye eşleyebilir, eşlemeyi kaldırabilir ya da bir bütün aralığı
tek seferde eşleyerek her sayfa için ayırıcıdan taze çerçeveler çekebilir. Ayrıca
sayfaları kullanıcı erişimine açık olarak işaretlemeyi de bilir; bu, çekirdeğin
belleği ring-3 userland'a açması gerektiğinde önem kazanır.

Burada tekrar eden bir ayrıntı, işlemcinin yazma koruması (write-protect) bitiyle
yapılan küçük bir danstır. Çekirdek yazma koruması etkin hâlde çalışır; bu normalde
ring-0 kodunun bile salt okunur eşlemeler üzerinden yazmasını engeller. Sayfa
tablolarını güvenle düzenlemek için yönetici bu biti kısaca temizler, eşlemeyi
yapar ve geri yükler — her değişikliği parantez içine alarak korumanın yalnızca bir
an için kapalı kalmasını sağlar.

## Heap

En üst katman, `mm/heap`'te, Rust'ın `alloc` tiplerinin dayandığı küçük ve sık
ayırmaları karşılayan bir bağlı liste ayırıcısıdır — çekirdekteki her `Vec`,
`String` ve `Box`. İlk kullanımında sabit yüksek bir adreste bir megabaytlık bir
bölge eşler ve serbest listesini o tek blokla tohumlar. O andan itibaren ayırma,
listede yeterince büyük bir blok arar, ihtiyaç duyduğunu ayırır ve geri kalanı
havuza döndürür; serbest bırakma ise bölgeyi listeye geri iter.

Çekirdek ve userland'ın her biri kendi sabit adresinde kendi heap'ine sahiptir —
çekirdeğinki kendi adres alanında yüksekte, userland'ınki daha da yüksekte. Hiçbir
durumu paylaşmazlar; bu da ayrıcalıklı ve ayrıcalıksız kod arasındaki sınırı temiz
tutar: userland, çekirdeğin onun için eşlediği bellekten ayırma yapar ve çekirdeğin
kendi havuzuna asla dokunmaz.

```
   heap (Vec, String, Box)          ← küçük nesneler
        │ içine eşlenir
   sayfa tablosu yöneticisi (sanal → fiziksel)
        │ çerçeveler buradan
   çerçeve ayırıcısı (RAM'in 4 KB sayfalarının bitmap'i)
```