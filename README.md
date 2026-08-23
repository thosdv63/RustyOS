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

Rusty OS (displayed internally as **T-OS 3.0**) is a `no_std`, bare-metal operating
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

Rusty OS (T-OS 3.0), x86_64 mimarisi için yazılmış,
`no_std` ve bare-metal bir işletim sistemidir. Kendi UEFI önyükleyicisiyle
başlar; bellek yönetimi, ACPI/APIC, bir zamanlayıcı, depolama ve USB sürücüleri,
bir FAT32 dosya sistemi, bir kayıt defteri ve ring-3 bir userland içeren tam bir
çekirdeği ayağa kaldırır ve gerçekten kullanılabilir uygulamalarla dolu, Aero
tarzı bir masaüstüne ulaşır. Tamamı Rust ile yazılmıştır; Windows 7'nin
görünümünden ilham alır ve sıcak turuncu bir "Rusty" paletiyle tasarlanmıştır.

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

<div align="center">

*Built with Rust, on bare metal, one specification at a time.*

</div>
