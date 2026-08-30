# 03 — Bellek

Çekirdek kontrolü ele aldığında, bellek çalışmadan neredeyse başka hiçbir şey olamaz. Her ayırma, her sürücü tamponu, ekrandaki her pencere nihayetinde bellek alt sisteminden gelir ve Rusty OS bunu üç katmanda inşa eder: RAM'i Buddy Allocator algoritmasıyla yöneten fiziksel çerçeve ayırıcısı, 4 seviyeli sayfa tablolarını ve dev sayfaları yönlendiren sanal bellek yöneticisi (VMM) ve eşlenmiş sanal bölgeden dinamik nesneler yontan heap ayırıcısı.

## Fiziksel çerçeve ayırıcısı

En alt katman, `mm/pfa`'da, fiziksel belleği **Buddy Allocator** algoritmasıyla yönetir. Bitmap yaklaşımı yerine bellek, 0 ile 20 arasındaki seviyelere (`MAX_ORDER = 20`) bölünmüş blok listeleriyle izlenir — Order 0 tek bir 4 KB'lik sayfaya karşılık gelirken, Order 9 (2 MB) ve Order 18 (1 GB) gibi daha büyük bloklar tek seferde ayrılabilir. Başlangıçta önyükleyicinin `BootInfo` üzerinden ilettiği bellek haritasını tarar, kullanılabilir (`usable == 1`) bölgeleri hizalayarak uygun seviyedeki serbest listelere (`free_lists`) ekler.

Donanım uyumluluğu ve sistem kararlılığı için ilk 1 MB'lık fiziksel bellek bölgesi (`0x100000` öncesi) tamamen ayırıcının dışında tutulur. Fiziksel bellekteki serbest blok yapılarının yönetimi ise **HHDM (Higher Half Direct Map)** tekniğiyle sağlanır; `0xFFFF_8000_0000_0000` offset'i kullanılarak fiziksel adresler sanal adres alanına dönüştürülür ve serbest blok düğümleri (`FreeBlock`) bellek üzerinde doğrudan manipüle edilir.

## Sayfa tablosu yöneticisi

Çerçeve ayırıcısının üzerinde `mm/vmm`, 4 seviyeli (PML4, PDPT, PD, PT) x86_64 sanal bellek mimarisini yönetir. Sanal adresleri fiziksel çerçevelere eşleme (`map`), eşleme kaldırma (`unmap`) ve adres çevirisi (`translate`) işlemlerini yürütür. Yapı; standart 4 KB sayfaların yanı sıra yüksek performans gerektiren durumlar için 2 MB ve 1 GB'lık **Dev Sayfaları (Huge Pages)** destekler.

Sanal bellek yöneticisi, alt sayfa tablolarına doğrudan HHDM offset'i üzerinden erişir. Güvenlik tarafında, **NXE (No-Execute Enable)** özelliği aktifleştirilerek `PTE_NO_EXECUTE` biti ile veri bölgelerinde kod çalıştırılması engellenir. Ayrıca `PTE_USER` (Ring-3 erişimi), `PTE_WRITABLE` ve önbellek kapatma (`PTE_NO_CACHE`, `PTE_WRITE_THROUGH`) bayrakları ile bellek erişimleri hassas bir şekilde denetlenir. Her tablo değişikliğinde TLB `invlpg` talimatı ile güncellenir.

## Heap

En üst katman, `mm/heap`'te, Rust'ın `alloc` tiplerine (`Vec`, `String`, `Box`) hizmet eden ve `spin::Mutex` ile senkronize edilen `LockedHeap` ayırıcısıdır. Rust'ın `GlobalAlloc` arayüzünü uygular.

Tembel yükleme (lazy initialization) mantığıyla çalışan heap, ilk ayırma isteği geldiğinde `0x4444_0000_0000` (`HEAP_START`) sanal adresinde 1 MB'lık (`HEAP_SIZE`) bir bölgeyi `vmm::map_range` ile fiziksel çerçevelere bağlar. İç mimaride bağlı liste (`FreeBlock`) yapısını kullanır; gelen bellek isteklerini boyut ve hizalama (`align_up`) gereksinimlerine göre en uygun serbest bloktan ayırır, artık kalan parçaları tekrar havuza serbest blok olarak ekler.

```
   heap (Vec, String, Box)           ← LockedHeap (0x4444_0000_0000 / 1 MB)
       │ VMM map_range ile eşlenir
   sanal bellek yöneticisi (VMM)     ← 4 Seviyeli Tablo (4KB / 2MB / 1GB, NXE, HHDM)
       │ çerçeveler buradan çekilir
   çerçeve ayırıcısı (Buddy Alloc)   ← Order 0-20 (4KB - 4GB), HHDM tabanlı

```
