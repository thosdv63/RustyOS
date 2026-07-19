# 06 — Dosya Sistemi

Bir disk sürücüsü ham bayt bloklarını taşır; bir dosya sistemi ise bu blokları
adları olan dosyalara ve dizinlere dönüştürür. Rusty OS bunun için FAT32 kullanır;
iyi anlaşıldığı, evrensel olarak desteklendiği ve — önemlisi — EFI sistem
bölümünün kullandığı formatla aynı olduğu için seçilmiştir; bu da kurulum
ortamının, gerçek bir firmware'in tanıyacağı disklere önyükleyiciler yazmasına
olanak tanır. Buradaki her şey `fs` altında bulunur.

## Blok aygıtı soyutlaması

Temelde, `fs`'te tanımlanan `BlockDevice` trait'i vardır: bir blok oku, bir blok
yaz, blok boyutunu bildir. Sistemdeki her depolama sürücüsü — NVMe, AHCI ve USB
yığın depolama — bunu uygular; bu da dosya sistemi kodunun bu tek arayüze göre
yazıldığı ve altında ne tür bir donanım olduğunu asla bilmesine gerek olmadığı
anlamına gelir. Aynı FAT32 uygulamasının dahili bir SSD'de, bir SATA sürücüsünde
ya da bir USB bellekte birebir aynı şekilde çalışmasını sağlayan dikiş yeri budur.

## FAT32

`fs/fat32`'deki FAT32 uygulaması, diskin geometrisini öğrenmek için önyükleme
sektörünü okur — sektör başına bayt, küme başına sektör, dosya ayırma tablosunun
nerede başladığı, veri bölgesinin nerede başladığı — ve sonrasında geri kalan her
şey küme zincirlerini gezmekten ibarettir. Dizinler, zincirleri takip edilerek ve
dizin girdileri çözülerek okunur; eski 8.3 sınırından daha uzun adların ele
alınması için uzun dosya adı girdileri de dahil. Bir dosya yazmak kümeler ayırır,
onları FAT içinde birbirine bağlar ve ilk kümeyi işaret eden bir dizin girdisi
yazar.

Vurgulamaya değer bir ayrıntı, bir dosya yazmanın onun hangi dizine ait olduğunu —
üst kümesini — bilmesi gerektiğidir; her zaman kökte bulunduğunu varsaymak yerine.
Dosya sisteminin iç içe klasörleri doğru şekilde desteklemesini sağlayan şey budur
ve bu, ancak iki seviye derinde bir dosya kaydetmeye çalışıp onun yanlış yere
düştüğünü gördüğünüzde apaçık hâle gelen türden bir şeydir.

## Sanal dosya sistemi katmanı

FAT32'nin üzerinde, `fs/vfs`'te, herhangi bir dosya sisteminin sağlaması
gerekenleri tanımlayan küçük bir VFS bulunur: bir dosyayı ya da dizini okumak,
yazmak ve tanımlamak için bir `INode` trait'i ve kökü bulmak için bir `FileSystem`
trait'i. Bunun üzerinde `fs/file`, bir inode'u bir ofsetle sarar ve bir dosya
boyunca geçerli bir konumu izleyen tanıdık okuma, yazma ve seek işlemlerini sağlar.
Bu katman incedir ama çekirdeğin geri kalanının ve userland'ın dosya işlemlerinin
üzerine inşa edildiği soyutlamadır.

## Bölümler ve GPT

En güvenlik açısından kritik parça, Rusty OS'un bölümlenmiş diskleri nasıl ele
aldığıdır. Gerçek bir bilgisayarın diski tek bir FAT32 birimi değildir — asla
dokunulmaması gereken Windows, Linux ve EFI bölümlerine sahip, GPT ile bölümlenmiş
bir disktir. İki modül bunu güvenli hâle getirir.

`fs/offset`, bir GPT bölümünü sanki bağımsız bir diskmiş gibi sunan bir
`PartitionDevice` tanımlar. Her blok erişimine bölümün başlangıç ofsetini ekler ve
— işte önemli kısım — bölümün sınırının ötesindeki herhangi bir erişimi reddeder.
Sınır denetimi aygıtın kendisinde bulunduğu için, bir bölüme yönelik bir yazmanın
başka bir bölüme düşmesi matematiksel olarak imkânsızdır.

`fs/gpt`, GPT'yi okur: başlığı ve bölüm girdilerini ayrıştırır, her bölümün türünü
GUID'inden çözer ve sınıflandırır — bir EFI sistem bölümü, bir Microsoft ayrılmış
bölümü, bir Windows NTFS birimi, bir Linux dosya sistemi vb. Bu sınıflandırma,
kurulum ortamının güvenlik mantığını yönlendirir ve kullanıcının seçemeyeceği
korumalı bölümleri işaretler. Ayrıca, bir bütün diski FAT olarak ele almaya
çalışmadan önce GPT'yi tespit eden akıllı mount'u da sağlar — ki bu, koruyucu bir
MBR'nin bir dosya sistemiyle karıştırılmasını önler — ve kurulum ortamının bir
önyükleyici eklemesi gerektiğinde EFI sistem bölümünü bulur.

Çalışan sistem genelinde, bağlanmış disklere sırayla sürücü harfleri atanır: NVMe
diski `C:`, bir AHCI diski `D:` olur ve USB sürücüleri `E:`'den itibaren harfler
alır.