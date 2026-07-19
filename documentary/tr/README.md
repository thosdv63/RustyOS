# Rusty OS — Dokümantasyon (Türkçe)

Rusty OS'un, işlemcinin çalıştırdığı ilk komuttan masaüstünde çalışan uygulamalara
kadar, sade bir dille eksiksiz bir anlatımı. Tam resim için baştan sona okuyun ya da
ilgilendiğiniz alt sisteme atlayın.

*For English: [`../en/`](../en/)*

## İçindekiler

1. [Genel Bakış](00-genel-bakis.md) — Rusty OS nedir ve neden var
2. [Mimari](01-mimari.md) — workspace ve parçaların nasıl birleştiği
3. [Açılış Süreci](02-acilis-sureci.md) — firmware'den çekirdek girişine
4. [Bellek](03-bellek.md) — çerçeve ayırıcısı, sayfalama ve heap
5. [CPU ve Kesmeler](04-cpu-ve-kesmeler.md) — tanımlayıcı tabloları, APIC, ACPI, zamanlayıcı, syscall'lar
6. [Sürücüler](05-suruculer.md) — PCI, depolama, USB, giriş, ses
7. [Dosya Sistemi](06-dosya-sistemi.md) — FAT32, VFS, GPT, bölüm güvenliği
8. [Kayıt Defteri](07-registry.md) — ayarlar, kalıcılık, kurtarma
9. [Grafik](08-grafik.md) — framebuffer, renderer, imleç
10. [Userland ve Syscall](09-userland-ve-syscall.md) — core.bin, syscall ABI'si, olay döngüsü
11. [Masaüstü Ortamı](10-masaustu-ortami.md) — pencereler, görev çubuğu, başlat menüsü, OOBE, giriş
12. [Uygulamalar](11-uygulamalar.md) — yerleşik programlar
13. [RPE Kurulum](12-rpe-kurulum.md) — gerçek donanıma kurulum
14. [Derleme ve Çalıştırma](13-derleme-ve-calistirma.md) — araç zinciri, QEMU, make hedefleri
15. [Bilinen Sorunlar](14-bilinen-sorunlar.md) — pürüzlü kenarlar ve yol haritası