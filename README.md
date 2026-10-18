# BT_Keys_Importer

Import Bluetooth link keys from a Windows installation into BlueZ so that
already-paired devices keep working after dual-booting into Linux.

## Requirements

- Linux with BlueZ and `systemd`
- `chntpw` available in `PATH`
- Root privileges (needed to write to
  `/var/lib/bluetooth`)
- The Windows partition mounted read-write, or at least readable

Install `chntpw` on common distros:

```sh
sudo apt install chntpw      # Debian / Ubuntu
sudo dnf install chntpw      # Fedora
sudo pacman -S chntpw        # Arch
```

## Build

```sh
cargo build --release
```

The binary is produced at `target/release/BT_Keys_Importer`.

## Importing the keys

1. First pair the device on Linux.
2. Then boot into windows and pair it.
3. Boot back into Linux **without re-pairing** the device.
4. Run this tool to copy the Windows link key over to Linux.

## Run

> Run it as **root**

```sh
sudo ./target/release/BT_Keys_Importer
```

Example:

```text
Enter mounting point of windows dir (e.g. /mnt/windows): /mnt/windows
0: 00112233AABB
Enter the bluetooth mac(first is 0): 0
0: AABBCCDDEEFF
Enter the bluetooth device mac(first is 0): 0
```

Once finished executing the binary should:

1. Stop the `bluetooth` service.
2. Read the selected link key from the Windows registry.
3. Write it to `/var/lib/bluetooth/<adapter>/<device>/info`.
4. Restart the `bluetooth` service.
