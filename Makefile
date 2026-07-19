
KERNEL_TARGET = x86_64-unknown-none
BOOT_TARGET = x86_64-unknown-uefi
KERNEL_BIN = target/$(KERNEL_TARGET)/debug/kernel
BOOT_EFI = target/$(BOOT_TARGET)/debug/boot.efi
OVMF_CODE = /usr/share/OVMF/OVMF_CODE_4M.fd
OVMF_VARS = /usr/share/OVMF/OVMF_VARS_4M.fd
USERLAND_BIN = userland/core.bin
IMG = rusty.img
ISO_IMG = rusty.iso

all: run


userland:
	cd userland && cargo build --release --target x86_64-unknown-none && rust-objcopy -O binary target/x86_64-unknown-none/release/core core.bin
	test -f userland/core.bin || (echo "HATA: userland/core.bin uretilemedi!" && exit 1)


kernel: userland
	rm -f $(KERNEL_BIN)
	rm -f target/x86_64-unknown-none/debug/deps/kernel-*
	cargo build --package kernel --target x86_64-unknown-none


boot:
	cargo build --package boot --target $(BOOT_TARGET)

image: kernel boot
	dd if=/dev/zero of=$(IMG) bs=1M count=256
	mformat -i $(IMG) -F ::
	mmd -i $(IMG) ::/EFI
	mmd -i $(IMG) ::/EFI/BOOT
	mcopy -i $(IMG) $(BOOT_EFI) ::/EFI/BOOT/BOOTX64.EFI
	mcopy -i $(IMG) $(KERNEL_BIN) ::/kernel.elf
	mcopy -i $(IMG) $(USERLAND_BIN) ::/core.bin
	dd if=/dev/zero of=nvme_disk.img bs=1M count=256
	dd if=/dev/zero of=sata_disk.img bs=1M count=256
	mkfs.fat -F 32 nvme_disk.img
	mkfs.fat -F 32 sata_disk.img
	mmd -i nvme_disk.img ::/RSYS
	mmd -i nvme_disk.img ::/Apps
	printf "Sistem/Ad=str:Rusty OS\nSistem/Surum=str:0.1\nSistem/Dil=str:tr\nSistem/Masaustu/Renk=u32:5249032\nSistem/Taskbar/Renk=u32:2101256\nSistem/Saat/UTC=u32:3\nSistem/Saat/24Saat=bool:0\nSistem/Ses/Seviye=u32:100\nOturum/IlkKurulumBitti=bool:0\n" > /tmp/registry.dat
	truncate -s 4096 /tmp/registry.dat
	mcopy -i nvme_disk.img /tmp/registry.dat ::/RSYS/REGISTRY.DAT
	mmd -i sata_disk.img ::/RSYS
	mmd -i sata_disk.img ::/Apps
	printf "Sistem/Ad=str:Rusty OS\nSistem/Surum=str:0.1\nSistem/Dil=str:tr\nSistem/Masaustu/Renk=u32:5249032\nSistem/Taskbar/Renk=u32:2101256\nSistem/Saat/UTC=u32:3\nSistem/Saat/24Saat=bool:0\nSistem/Ses/Seviye=u32:100\nOturum/IlkKurulumBitti=bool:0\n" > /tmp/registry.dat
	truncate -s 4096 /tmp/registry.dat
	mcopy -i sata_disk.img /tmp/registry.dat ::/RSYS/REGISTRY.DAT
	mcopy -i nvme_disk.img $(USERLAND_BIN) ::/RSYS/CORE.BIN
	mcopy -i nvme_disk.img $(KERNEL_BIN) ::/RSYS/KERNEL.ELF
	mcopy -i sata_disk.img $(USERLAND_BIN) ::/RSYS/CORE.BIN
	mcopy -i sata_disk.img $(KERNEL_BIN) ::/RSYS/KERNEL.ELF
	dd if=/dev/zero of=usb_disk.img bs=1M count=256
	mkfs.fat -F 32 usb_disk.img
	mmd -i usb_disk.img ::/Belgelerim
	printf "Rusty OS USB testi\n" > /tmp/usb_test.txt
	mcopy -i usb_disk.img /tmp/usb_test.txt ::/OKUBENI.TXT
	mcopy -i nvme_disk.img kernel/src/win7.raw ::/SES.RAW
	mcopy -i sata_disk.img kernel/src/win7.raw ::/SES.RAW

run: image
	cp $(OVMF_VARS) /tmp/rusty_ovmf_vars.fd
	qemu-system-x86_64 -machine q35,accel=kvm:tcg \
		-cpu Skylake-Client \
		-smp 4,cores=2,threads=2,sockets=1 \
		-m 2048M \
		-d cpu_reset \
		-drive if=pflash,format=raw,readonly=on,file=$(OVMF_CODE) \
		-drive if=pflash,format=raw,file=/tmp/rusty_ovmf_vars.fd \
		-drive format=raw,file=$(IMG) \
		-serial stdio \
		-audiodev pa,id=snd0 \
		-device ich9-intel-hda,id=hda \
		-device hda-duplex,bus=hda.0,audiodev=snd0 \
		-device AC97,audiodev=snd0 \
		-drive file=nvme_disk.img,if=none,id=nvm,format=raw \
		-device nvme,serial=deadbeef,drive=nvm \
		-drive file=sata_disk.img,if=none,id=sata0,format=raw \
		-device ich9-ahci,id=ahci \
		-device ide-hd,drive=sata0,bus=ahci.0 \
		-device qemu-xhci,id=xhci \
		-device usb-kbd,bus=xhci.0 \
		-device usb-mouse,bus=xhci.0 \
		-drive file=usb_disk.img,if=none,id=usbstick,format=raw \
		-device usb-storage,bus=xhci.0,drive=usbstick \
		-display gtk,zoom-to-fit=on
	

NVME_GPT := nvme_disk.img


QEMU := qemu-system-x86_64 -machine q35,accel=kvm:tcg -cpu Skylake-Client -smp 4 -m 2048M \
	-drive if=pflash,format=raw,readonly=on,file=$(OVMF_CODE) \
	-drive if=pflash,format=raw,file=/tmp/ovmf_vars.fd \
	-audiodev pa,id=snd0 \
	-device ich9-intel-hda,id=hda -device hda-duplex,bus=hda.0,audiodev=snd0 \
	-device AC97,audiodev=snd0 \
	-device qemu-xhci,id=xhci


PAYLOAD_DIR := kernel/src/kernel/rpe/payload

rpe-usb: userland
	mkdir -p $(PAYLOAD_DIR)
	printf 'STUB' > $(PAYLOAD_DIR)/BOOTX64.EFI
	printf 'STUB' > $(PAYLOAD_DIR)/KERNEL.ELF
	printf 'STUB' > $(PAYLOAD_DIR)/CORE.BIN
	cargo build --package boot --target x86_64-unknown-uefi
	cargo build --package kernel --target x86_64-unknown-none
	@echo ">>> Ham kernel:"; ls -lh $(KERNEL_BIN)
	# Kurulacak kernel: STRIP (debug sembolleri ~20MB)
	cp $(KERNEL_BIN) /tmp/rusty_pass1.elf
	strip -s /tmp/rusty_pass1.elf
	cp $(BOOT_EFI) $(PAYLOAD_DIR)/BOOTX64.EFI
	cp /tmp/rusty_pass1.elf $(PAYLOAD_DIR)/KERNEL.ELF
	cp $(USERLAND_BIN) $(PAYLOAD_DIR)/CORE.BIN
	@echo ">>> Gomulecek payload:"; ls -lh $(PAYLOAD_DIR)/
	# GECIS 2: RPE kernel (payload gomulu)
	cargo build --package kernel --target x86_64-unknown-none
	cp $(KERNEL_BIN) /tmp/rusty_rpe.elf
	strip -s /tmp/rusty_rpe.elf
	@echo ">>> RPE kernel (USB'ye giden):"; ls -lh /tmp/rusty_rpe.elf
	dd if=/dev/zero of=rpe_usb.img bs=1M count=256 status=none
	mkfs.fat -F32 rpe_usb.img >/dev/null
	mmd -i rpe_usb.img ::/EFI ::/EFI/BOOT
	mcopy -i rpe_usb.img $(BOOT_EFI) ::/EFI/BOOT/BOOTX64.EFI
	mcopy -i rpe_usb.img /tmp/rusty_rpe.elf ::/kernel.elf
	printf "RPE" > /tmp/rpe.flag
	mcopy -i rpe_usb.img /tmp/rpe.flag ::/RPE.FLAG

gpt-disk: boot
	dd if=/dev/zero of=$(NVME_GPT) bs=1M count=4096 status=none
	sgdisk -og $(NVME_GPT)
	sgdisk -n 1:2048:+100M -t 1:EF00 -c 1:"EFI System"         $(NVME_GPT)
	sgdisk -n 2:0:+16M     -t 2:0C01 -c 2:"Microsoft reserved" $(NVME_GPT)
	sgdisk -n 3:0:+1G      -t 3:0700 -c 3:"Basic data"         $(NVME_GPT)
	sgdisk -n 4:0:+1G      -t 4:8300 -c 4:"Linux filesystem"   $(NVME_GPT)
	sgdisk -n 5:0:+1500M   -t 5:0700 -c 5:"RUSTY"              $(NVME_GPT)
	@set -e; \
	LOOP=$$(sudo losetup -f -P --show $(NVME_GPT)); \
	echo ">>> loop: $$LOOP"; \
	sudo partprobe $$LOOP || true; \
	sleep 1; \
	sudo mkfs.fat -F32 $${LOOP}p1 >/dev/null; \
	sudo mkdir -p /tmp/esp; \
	sudo mount $${LOOP}p1 /tmp/esp; \
	sudo mkdir -p /tmp/esp/EFI/BOOT; \
	sudo cp $(BOOT_EFI) /tmp/esp/EFI/BOOT/BOOTX64.EFI; \
	sudo umount /tmp/esp; \
	printf '\xEB\x52\x90NTFS    ' | sudo dd of=$${LOOP}p3 bs=1 count=11 conv=notrunc status=none; \
	sudo mkfs.fat -F32 $${LOOP}p5 >/dev/null; \
	sudo losetup -d $$LOOP; \
	echo ">>> GPT disk hazir: 1=ESP 2=MSR 3=Windows 4=Linux 5=RUSTY(hedef)"


run-rpe: rpe-usb gpt-disk
	cp $(OVMF_VARS) /tmp/ovmf_vars.fd
	$(QEMU) \
		-drive file=$(NVME_GPT),if=none,id=nvm,format=raw \
		-device usb-mouse,bus=xhci.0 \
		-device nvme,serial=deadbeef,drive=nvm,bootindex=1 \
		-drive file=rpe_usb.img,if=none,id=usb,format=raw \
		-device usb-storage,bus=xhci.0,drive=usb,bootindex=0 \
		-display gtk,zoom-to-fit=on


run-installed:
	cp $(OVMF_VARS) /tmp/ovmf_vars.fd
	$(QEMU) \
		-drive file=$(NVME_GPT),if=none,id=nvm,format=raw \
		-device nvme,serial=deadbeef,drive=nvm \
		-device usb-mouse,bus=xhci.0 \
		-display gtk,zoom-to-fit=on
clean:
	cargo clean
	rm -f $(IMG) nvme_disk.img sata_disk.img usb_disk.img rpe_usb.img $(USERLAND_BIN)

.PHONY: all userland kernel boot image run rpe-image run-rpe run-installed clean