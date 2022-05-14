# Custom Rust Kernel bauen

## 1. Vorbereitung

Kernel holen (z.B. [`github.com/niklasmohrin/linux`](https://github.com/niklasmohrin/linux)), [Arch Iso holen](https://archlinux.org/download/) (relativ neu, mit Installer)

## 2. Kernel bauen

Siehe [`quick-start.rst`](https://github.com/niklasmohrin/linux/blob/ramfs/Documentation/rust/quick-start.rst) für Toolchain.

Wir haben die zwei Skripte `b` und `scp_all` zusammengestellt. Insbesondere ist `b` ein Wrapper um den `make` command, der die Compiler Optionen setzt (also `rustup default` nicht verändert werden muss). Falls man die nicht benutzen möchte, hier unser vorherige Anleitung:

```console
$ mkdir -p builds/{kernel, modules}
$ cd mylinuxclone
$ make O=../builds/kernel defconfig                      # wir wollen das repo selbst nicht zum bauen benutzen
$ cd ../builds/kernel
$ rustup default nightly-2021-05-29
$ make menuconfig                                        # Rust Support auswählen, Rest erstmal egal
$ make ARCH=x86_64 CC=clang -j12                         # baut Kernel und gewählte Module
...
something about bzImage                                  # irgendwie sowas sollte am Ende stehen
...
$ make INSTALL_MOD_PATH=../modules modules_install       # Alle Module sammeln
```

## 3. VM aufbauen

- qemu und ovmf installieren (man braucht vor allem eine `OVMF.fd`)
- `qemu-img create -f qcow2 archinstaller-clean.qcow2 8G`
- VM launchen mit Befehl unten, wobei das `-cdrom ...` mit benutzt werden muss
- `archinstall` guided installer, Profil minimal (kein XOrg)
- root account passwort muss gesetzt werden für SSH
- falls später was fehlt `pacman -S neovim openssh tree` oder so

## 4. Snapshots

Jetzt bietet es sich an, einen Snapshot zu machen:

```
qemu-img create -b archinstaller-clean.qcow2 snapshot1.qcow2
```

Hierbei ist wichtig, dass der Snapshot nicht mehr funktioniert, falls das Basis Image verändert wird (also nichtmal booten am besten). Falls man wieder da anfangen will, sollte ein neuer Snapshot mit der gleichen Basis erstellt werden.

Es gibt auch noch die `-snapshot` Flag beim starten, da arbeitet man immer nur auf einem Snapshot der nach Ende der Session weggeschmissen wird (wir wissen aber nicht, ob es safe ist das Basis Image damit zu booten).

## 5. VM launchen

Hier sollte das `pool/current.qcow2` durch Pfad zum Disk Image und `OVMF.fd` durch vollen Pfad zur Datei ersetzt werden

```sh
#!/bin/sh

qemu-system-x86_64 \
    -drive file=pool/current.qcow2,format=qcow2 \
    -boot menu=on \
    -bios OVMF.fd \
    -nic user,hostfwd=tcp::2222-:22 \
    -m 4096 \
    -cpu host \
    -enable-kvm
    # -nographic \
    # -cdrom archlinux-2021.05.01-x86_64.iso \
    # -snapshot \
    # -vga virtio -display sdl,gl=on \
    # -fsdev local,id=fs1,path=share,security_model=none \
    # -device virtio-9p-pci,fsdev=fs1,mount_tag=shared_folder \
    # -kernel ../linux/arch/x86/boot/bzImage \
    # -append "console=ttyS0" \
```

## 6. SSHD konfigurieren

Im Guest in `/etc/ssh/sshd_config` (`pacman -S openssh`) die Option `PermitRootLogin yes` setzen.
Dann noch mit `systemctl enable --now sshd` den Server aktivieren.

## 7. Kernel und Module im Guest installieren

Bevor wir `builds/modules` rekursiv in die VM kopieren, sollten wir den Symlink auf den Sourcecode entfernen, weil sonst kopieren wir den mit. WICHTIG: nicht die Pfade mit `/` enden lassen, sonst wird das Verzeichnis hinter dem Link gelöscht.

```console
$ rm builds/modules/{source, build}
$ scp    -P 2222 builds/kernel/<something about x86>/bzImage  root@127.0.0.1:/boot/vmlinuz-rust
$ scp -r -P 2222 builds/modules/lib/*                         root@127.0.0.1:/lib/
```

## 8. Boot Manager anpassen

Der Arch Installer hat uns systemd-boot installiert.

```console
$ cd /boot/loader/entries/
$ cp <einzige Datei hier> linux-rust.conf
$ nvim linux-rust.conf
<Titel und Image ändern>
ESC ESC :wq<Enter> here you go
$ bootctl install
$ reboot
```

## 9. Testen

Am besten man hat `rust_minimal` als eingebautes Modul kompiliert, dann kann man nach dem reboot in `dmesg | less` nach Rust suchen (type: `/rust`, dann `n` zum nächsten Suchergebnis).
Mit `modprobe rust_print` wird das dynamische Modul geladen, das sollte jetzt in `dmesg | tail` und `lsmod | grep rust` auftauchen.

Kernelmodule die nicht in `/lib/modules/.../` (`...` sollte durch den Output von `uname -r` / die Versionsnummer ersetzt werden; in unserem Fall `5.12.0-rc4+`) liegen, können mit `insmod example.ko` und `rmmod example.ko` geladen und entladen werden.
