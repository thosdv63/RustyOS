# 01 — Mimari

Rusty OS, üç üyeli bir Cargo workspace olarak düzenlenmiştir — `kernel`, `boot` ve
`common` — artı kendi başına derlenip sisteme düz bir ikili olarak gömülen ayrı
bir `userland` paketi. Bunları ayrı derleme birimleri olarak tutmak önemlidir;
çünkü çok farklı dünyalarda çalışırlar: önyükleyici UEFI firmware servisleri
altında, çekirdek ring 0'da bare-metal, userland ise ayrıcalıksız bir ring-3
süreci olarak çalışır. Bir çalışma zamanını paylaşamazlar, bu yüzden kod bu
sınırlar boyunca bölünmüştür.

Gerçekten paylaşmaya ihtiyaç duydukları tek şey, önyükleyicinin çekirdeğe verdiği
verinin biçimidir. Bu, kasıtlı olarak minik tutulmuş `common` crate'inde bulunur:
`BootInfo` yapısını tanımlar — framebuffer açıklaması, bellek haritası, ACPI
işaretçisi ve sistemin bir kurulum ortamı olarak çalışıp çalışmadığını belirten
bir bayrak — ve pek fazla başka bir şey içermez. Hem `boot` hem de `kernel`,
`common`'a bağımlıdır; böylece önyükleyici bir `BootInfo` doldurur, çekirdek de
tam olarak aynı yerleşimi geri okur. Bu çizgiyi başka hiçbir şey geçmez.

## Yürütme akışı

Açılıştan çalışan bir masaüstüne kadar kontrol, dört ayrı aşamadan geçer:

```
  UEFI Firmware
       │  yükler ve başlatır
       ▼
  Önyükleyici (boot crate, ring 0, UEFI servisleri)
       │  çekirdek ELF'ini yükler, BootInfo doldurur, boot servislerinden çıkar
       ▼
  Çekirdek (kernel crate, ring 0, bare metal)
       │  belleği, sürücüleri, dosya sistemini ayağa kaldırır; core.bin yükler
       ▼
  Userland (userland paketi, ring 3)
          masaüstü, pencereler ve uygulamalar
```

Her ok gerçek bir devir teslimdir. Önyükleyici, bir yazmaçta `BootInfo`'ya bir
işaretçiyle çekirdeğin giriş noktasına atlar. Çekirdek, her şey hazır olduğunda,
işlemciyi ring 3'e indirir ve userland ikilisini çalıştırmaya başlar. O andan
itibaren userland, çekirdeğe yalnızca sistem çağrıları aracılığıyla ulaşabilir.

## Kaynak nasıl düzenlenmiş

Çekirdek crate'i, kodun çoğunun yaşadığı yerdir ve sorumluluğa göre bölünmüştür.
Mimariye özgü kurulum — tanımlayıcı tabloları, kesme tablosu, ACPI — `arch`
altında bulunur. Donanım sürücüleri `drivers` altında gruplanmıştır; depolama,
USB, ses ve PS/2'nin her biri kendi alt ağacındadır. FAT32 uygulaması ve VFS
katmanıyla birlikte dosya sistemi `fs` altındadır. Daha üst düzey çekirdek
servisleri — zamanlayıcı, süreç ve syscall mekanizması, olay kuyruğu, kayıt
defteri ve kurulum ortamı — `kernel` altında yaşar. Bellek yönetiminin `mm`'de
kendi evi vardır ve erken önyükleme sırasında ve çekirdek panikleri için
kullanılan framebuffer renderer'ı kendi çizim modülünde bulunur.

Userland paketi bu yapının bir kısmını yansıtır ama çok daha basittir. Kendi heap
ayırıcısı, kendi renderer'ı, ince bir syscall sarmalayıcı kütüphanesi, bir
uygulama çerçevesi ve masaüstünü uygulayan UI katmanı vardır. Düz bir ikiliye
derlenip sabit bir adrese yüklendiği için, `main` çalışmadan önce kendini
hazırlamak üzere az miktarda düşük seviyeli başlangıç kodu da taşır.

## Adresler üzerine bir not

Çekirdek fiziksel belleği birebir eşlediği (identity-map) ve belirli amaçlar için
sabit sanal bölgeler dağıttığı için, kod tabanı boyunca birkaç adres tekrar tekrar
karşımıza çıkar. Sonraki bölümleri okurken akılda tutmakta fayda var:

| Bölge | Adres | Amaç |
|-------|-------|------|
| Userland / core.bin | `0x000000`–`0x500000` | rezerve; DMA burada asla ayırma yapmaz |
| Arka tampon | `0x10000000` | userland'ın ekrana basmadan önce çizdiği yer |
| Framebuffer | `0x80000000` | fiziksel ekran, arka tampondan kopyalanır |
| Çekirdek heap | `0x44440000000` | çekirdek için bağlı liste ayırıcısı |
| Userland heap | `0x50000000000` | userland için bağlı liste ayırıcısı |

Bunlar rastgele ayrıntılar değil — sistemi ayakta tutan unsurlar. Örneğin rezerve
edilmiş düşük bölge, tam da bir disk denetleyicisinden gelen bir DMA aktarımının
çalışan userland'ın üzerine asla yazamaması için vardır; olaydan sonra ayıklanması
neredeyse imkânsız olan bir hata sınıfı.