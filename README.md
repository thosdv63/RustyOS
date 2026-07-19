<div align="center">

# Rusty OS

**A complete x86_64 desktop operating system, written from scratch in pure Rust.**

No `std`. No existing kernel. No borrowed drivers. Just bare metal, a UEFI
bootloader, and a Windows 7-inspired desktop — all the way down.

</div>

---

*[English](#english) · [Türkçe](#türkçe)*

---

## English

Rusty OS (displayed internally as **T-OS**) is a `no_std`, bare-metal operating
system for the x86_64 architecture. It boots through its own UEFI bootloader,
brings up a full kernel — memory management, ACPI/APIC, a scheduler, storage and
USB drivers, a FAT32 filesystem, a registry, and a ring-3 userland — and lands
on an Aero-styled desktop with real, usable applications. The whole thing is
written in Rust and inspired by the look and feel of Windows 7, dressed in a warm
orange "Rusty" palette.

It also ships with the **Rusty Preinstallation Environment (RPE)**, a Windows
7-style installer that can write Rusty OS onto a real GPT disk without disturbing
an existing Windows, Linux, or EFI installation.

### What's inside

The kernel stack covers the full path from firmware to desktop:

| Layer | Highlights |
|-------|-----------|
| **Boot** | UEFI bootloader, Windows 7-style boot manager, ELF loader, dual-boot chainloading |
| **Core** | GDT/TSS, IDT, APIC/LAPIC, ACPI, physical + virtual memory, heap, scheduler |
| **Drivers** | PCI/HAL, NVMe, AHCI, xHCI (USB), HID, USB mass storage, PS/2, Intel HDA, AC'97, RTC |
| **Filesystem** | FAT32, a VFS layer, GPT parsing, partition-bounded block devices |
| **System** | SYSCALL/SYSRET ABI, ring-3 processes, a plain-text registry, a recovery mode |
| **Userland** | Window manager, taskbar, start menu, OOBE, login, and 10+ built-in apps |
| **Installer** | RPE — a self-contained installer that deploys onto real hardware |

The built-in applications include a File Explorer, Notepad, Paint, a Calculator,
a Registry Editor, a Task Manager, a Settings panel, a Command Prompt, an Image
Viewer, and an About window — each talking to the kernel through a small syscall
ABI.

### Quick start

You'll need a Linux host with the Rust nightly toolchain, QEMU (10.1+), OVMF
firmware, and `mtools`. From the project root:

```bash
# Build everything and boot Rusty OS in QEMU
make run

# Build the RPE installer image and boot it against a virtual GPT disk
make run-rpe

# Boot a system that has already been installed to the virtual disk
make run-installed
```

The first boot walks you through the OOBE setup wizard and a login screen before
dropping you on the desktop.

> **Note on real hardware:** Rusty OS has been successfully installed onto a real
> laptop's FAT32 partition via the RPE, booting through F12 without touching the
> existing Windows, Linux, or EFI setup. See the documentation for the exact
> procedure.

### Reading the documentation

Every subsystem is explained, in plain language, under the [`documentary/`](documentary/)
folder. The documentation is available in both **English** (`documentary/en/`) and
**Turkish** (`documentary/tr/`), and it's meant to be read start-to-finish or
dipped into by topic.

A good reading order:

1. **[Overview](documentary/en/00-overview.md)** — what Rusty OS is and why it exists
2. **[Architecture](documentary/en/01-architecture.md)** — the workspace and how the pieces fit
3. **[Boot Process](documentary/en/02-boot-process.md)** — from firmware to kernel entry
4. From there, follow your interest: memory, drivers, the filesystem, graphics,
   the desktop, the applications, and the RPE installer each have their own chapter.

If you just want to build and run, jump straight to
**[Building & Running](documentary/en/13-building-and-running.md)**. If you want an
honest account of what's still rough, read **[Known Issues](documentary/en/14-known-issues.md)**.

### Status

Version 1 of the OS is complete, and the RPE installer works on real hardware.
Development continues. Contributions, questions, and suggestions are welcome — see
the personal note below.

### License

Released under the MIT License. You are free to use, study, modify, and build on
this kernel however you like.

---

## Türkçe

Rusty OS (sistem içinde **T-OS** olarak görünür), x86_64 mimarisi için yazılmış,
`no_std` ve bare-metal bir işletim sistemidir. Kendi UEFI önyükleyicisiyle
başlar; bellek yönetimi, ACPI/APIC, bir zamanlayıcı, depolama ve USB sürücüleri,
bir FAT32 dosya sistemi, bir kayıt defteri ve ring-3 bir userland içeren tam bir
çekirdeği ayağa kaldırır ve gerçekten kullanılabilir uygulamalarla dolu, Aero
tarzı bir masaüstüne ulaşır. Tamamı Rust ile yazılmıştır; Windows 7'nin
görünümünden ilham alır ve sıcak turuncu bir "Rusty" paletiyle giydirilmiştir.

Ayrıca **Rusty Preinstallation Environment (RPE)** ile birlikte gelir; bu,
mevcut bir Windows, Linux veya EFI kurulumuna dokunmadan Rusty OS'u gerçek bir
GPT diske yazabilen, Windows 7 tarzı bir kurulum ortamıdır.

### İçinde neler var

Çekirdek yığını, firmware'den masaüstüne kadar tüm yolu kapsar:

| Katman | Öne çıkanlar |
|--------|-------------|
| **Önyükleme** | UEFI önyükleyici, Windows 7 tarzı önyükleme yöneticisi, ELF yükleyici, çoklu önyükleme |
| **Çekirdek** | GDT/TSS, IDT, APIC/LAPIC, ACPI, fiziksel + sanal bellek, heap, zamanlayıcı |
| **Sürücüler** | PCI/HAL, NVMe, AHCI, xHCI (USB), HID, USB depolama, PS/2, Intel HDA, AC'97, RTC |
| **Dosya sistemi** | FAT32, bir VFS katmanı, GPT ayrıştırma, bölüm sınırlı blok aygıtları |
| **Sistem** | SYSCALL/SYSRET ABI, ring-3 süreçler, düz metin kayıt defteri, kurtarma modu |
| **Userland** | Pencere yöneticisi, görev çubuğu, başlat menüsü, OOBE, giriş ve 10+ yerleşik uygulama |
| **Kurulum** | RPE — gerçek donanıma dağıtım yapan, kendi kendine yeten bir kurulum ortamı |

Yerleşik uygulamalar arasında bir Dosya Gezgini, Not Defteri, Paint, Hesap
Makinesi, Kayıt Düzenleyici, Görev Yöneticisi, Ayarlar paneli, Komut İstemi,
Resim Görüntüleyici ve bir Hakkında penceresi bulunur; her biri küçük bir
syscall ABI'si üzerinden çekirdekle konuşur.

### Hızlı başlangıç

Rust nightly araç zinciri kurulu bir Linux ana makinesine, QEMU'ya (10.1+),
OVMF firmware'ine ve `mtools`'a ihtiyacınız var. Proje kök dizininden:

```bash
# Her şeyi derle ve Rusty OS'u QEMU'da başlat
make run

# RPE kurulum imajını derle ve sanal bir GPT diske karşı başlat
make run-rpe

# Sanal diske zaten kurulmuş bir sistemi başlat
make run-installed
```

İlk açılış, sizi masaüstüne bırakmadan önce OOBE kurulum sihirbazından ve bir
giriş ekranından geçirir.

> **Gerçek donanım notu:** Rusty OS, RPE aracılığıyla gerçek bir dizüstü
> bilgisayarın FAT32 bölümüne başarıyla kurulmuş; mevcut Windows, Linux veya EFI
> kurulumuna dokunmadan F12 ile önyükleme yapmıştır. Tam prosedür için
> dokümantasyona bakın.

### Dokümantasyonu okumak

Her alt sistem, sade bir dille, [`documentary/`](documentary/) klasörü altında
anlatılır. Dokümantasyon hem **İngilizce** (`documentary/en/`) hem de **Türkçe**
(`documentary/tr/`) mevcuttur; baştan sona okunacak ya da konu konu göz atılacak
şekilde tasarlanmıştır.

İyi bir okuma sırası:

1. **[Genel Bakış](documentary/tr/00-genel-bakis.md)** — Rusty OS nedir ve neden var
2. **[Mimari](documentary/tr/01-mimari.md)** — workspace ve parçaların nasıl birleştiği
3. **[Açılış Süreci](documentary/tr/02-acilis-sureci.md)** — firmware'den çekirdek girişine
4. Buradan sonra ilginizi takip edin: bellek, sürücüler, dosya sistemi, grafik,
   masaüstü, uygulamalar ve RPE kurulumunun her birinin kendi bölümü var.

Sadece derleyip çalıştırmak istiyorsanız doğrudan
**[Derleme ve Çalıştırma](documentary/tr/13-derleme-ve-calistirma.md)** bölümüne
geçin. Neyin hâlâ pürüzlü olduğunu dürüstçe okumak isterseniz
**[Bilinen Sorunlar](documentary/tr/14-bilinen-sorunlar.md)** bölümüne bakın.

### Durum

İşletim sisteminin ilk sürümü tamamlandı ve RPE kurulumu gerçek donanımda
çalışıyor. Geliştirme devam ediyor. Katkılar, sorular ve öneriler memnuniyetle
karşılanır — aşağıdaki kişisel nota bakın.

### Lisans

MIT Lisansı altında yayımlanmıştır. Bu çekirdeği dilediğiniz gibi kullanmakta,
incelemekte, değiştirmekte ve üzerine bir şeyler inşa etmekte özgürsünüz.

---

## A note from the author / Yazardan bir not

### English

Hello. My name is Taha. I'm fourteen years old, and I'm one of the few young
people in my country working in Rust and low-level systems programming.

Your first reaction is probably surprise — *how does a fourteen-year-old write an
operating system alone?* I've grown up around computers for as long as I can
remember, and I was always the kind of kid who wanted to build things rather than
just use them. On my father's Lenovo B560 laptop, running Windows 7, I started by
making my own games in the Luau programming language. It was during that time —
around the transition from Windows 7 to Windows 10 — that I first became
fascinated by operating systems.

I took my first real step toward writing one two years ago, following
Absurdponcho's OSDev tutorial series. That project, which I called T-OS 1.0,
barely got a rendering system working before I set it aside. I then started T-OS
2.0 in C. I couldn't finish it because of school, but that's where I wrote my
first AHCI driver.

The thing that slowed me down most was the difficulty of the documentation. In
the world of operating systems, everything is exact and unforgiving. That's not a
trait I naturally enjoy — but I decided to wrestle with it anyway. I read the
specifications myself, my English improved along the way, and AI became a genuinely
useful assistant. For the last year I've been working on Rusty OS. I've been
writing operating systems for two and a half years, and I've wanted to write one
for nearly seven.

In early July 2026 I finally did it. I flashed my Rusty Preinstallation
Environment image onto a USB stick and installed Rusty OS onto my own Lenovo
IdeaPad 3. For me this was never just a hobby — it was the payoff of a
seven-year-long ambition. Along the way I learned the x86 architecture deeply and
spent countless hours on it. But as I said, I'm someone who chafes at how rigid
everything is. I'm a low-level person to the core, so alongside continuing to
develop this OS, I've set my sights on designing my own RISC-style architectures.

My door is fully open to anyone who wants to help develop this project. My inbox
is open; anyone with a suggestion can write to me. Soon I want to implement
networking drivers and pull data from internet addresses. And this OS has
convinced me of one thing: Rust really will overtake C.

This project will be one of the proudest entries on my future résumé, and anyone
is free to make use of it however they wish. Because I'm Turkish, some struct
field names and some output text may be in Turkish; I translated as much as I
could and left the userland in Turkish. Everyone is welcome to build on it — and I
believe this kernel is a fine foundation for writing entirely different userlands
on top of. It may not contain thousands of libraries like the Linux kernel, and it
may not include modern drivers, but if someone ever wonders *what would an OS like
this look like?*, they can use this kernel to find out.

The first step of my road toward Silicon Valley runs through this OS. Thank you to
everyone who read this far. On my GitHub, projects are coming where I've designed
my own RISC-style architecture and where I work at Ring -1 — the hypervisor layer.
Stay tuned. Have a good day, everyone.

### Türkçe

Merhabalar. Ben Taha. On dört yaşındayım ve ülkemde Rust ile kod yazıp low-level
sistemlerle uğraşan sayılı gençlerden biriyim.

İlk tepkiniz muhtemelen şaşkınlıktır — *on dört yaşında bir çocuk tek başına nasıl
işletim sistemi yazar?* Kendimi bildim bileli bilgisayarların arasında büyüdüm ve
her zaman tüketmekten çok üretmek isteyen bir çocuktum. Babamın Lenovo B560
dizüstü bilgisayarında, Windows 7 üzerinde, önce Luau programlama dilinde kendi
oyunlarımı yaptım. İşte o dönemde — Windows 7'den Windows 10'a geçiş sıralarında —
ilk kez işletim sistemlerine ilgi duymaya başladım.

İşletim sistemi yazma yolunda ilk gerçek adımı iki yıl önce, Absurdponcho'nun
OSDev eğitim serisini takip ederek attım. T-OS 1.0 adını verdiğim o projede zar
zor bir renderer yapısı kurabilmiş, ardından ara vermiştim. Sonra C ile T-OS 2.0'ı
yazmaya başladım. Eğitimim yüzünden bitiremedim, ama ilk AHCI sürücümü işte orada
yazdım.

Beni en çok yavaşlatan şey, dokümantasyonun zorluğuydu. İşletim sistemi
dünyasında her şey kesin ve affetmezdir. Bu, doğam gereği pek sevdiğim bir özellik
değil — ama yine de bu zorlukla boğuşmaya karar verdim. Belgeleri kendim okudum,
bu süreçte İngilizcem gelişti ve yapay zeka benim için gerçekten faydalı bir
asistan oldu. Son bir yıldır Rusty OS üzerinde çalışıyorum. İki buçuk yıldır
işletim sistemi yazıyorum ve neredeyse yedi yıldır bir tane yazmak istiyordum.

Temmuz 2026'nın başlarında sonunda başardım. Rusty Preinstall Environment imajımı
bir USB belleğe yazdım ve Rusty OS'u kendi Lenovo IdeaPad 3 bilgisayarıma kurdum.
Benim için bu asla sadece bir hobi olmadı — yedi yıllık bir hırsın karşılığıydı.
Bu yolda x86 mimarisini derinlemesine öğrendim ve saatlerce uğraştım. Ama dediğim
gibi, her şeyin bu kadar katı olmasından rahatsız olan biriyim. İliklerine kadar
low-level bir insanım; bu yüzden bu OS'u geliştirmeye devam etmenin yanında,
gözümü kendi RISC tarzı mimarilerimi tasarlamaya diktim.

Bu projeyi geliştirmek isteyen herkese kapım sonuna kadar açık. E-postam açık;
önerisi olan herkes bana yazabilir. Yakında ağ sürücüleri eklemek ve internet
adreslerinden veri çekmek istiyorum. Ve bu OS beni bir konuda ikna etti: Rust,
gerçekten C'yi devirecek.

Bu proje, gelecekteki özgeçmişimin en gurur duyduğum köşelerinden biri olacak ve
isteyen herkes dilediği gibi yararlanabilir. Türk olduğum için bazı struct alan
adları ve bazı çıktılar Türkçe olabilir; elimden geldiğince çevirdim, userland'ı
Türkçe bıraktım. Herkes üzerine bir şeyler inşa edebilir — ve bence bu çekirdek,
üzerine bambaşka userlandlar yazmak için güzel bir temeldir. Linux çekirdeği gibi
binlerce kütüphane içermeyebilir, modern sürücüleri barındırmayabilir; ama biri
bir gün *acaba böyle bir OS nasıl olurdu, nasıl görünürdü?* diye merak ederse, bu
çekirdeği kullanarak öğrenebilir.

Silikon Vadisi'ne giden yolumun ilk adımı bu OS'tan geçiyor. Buraya kadar okuyan
herkese teşekkür ederim. GitHub'ımda, kendi RISC tarzı mimarimi tasarladığım ve
Ring -1 — yani hipervizör katmanında — çalıştığım projeler geliyor. Beklemede
kalın. Herkese iyi günler.

---

<div align="center">

*Built with Rust, on bare metal, one specification at a time.*

</div>
