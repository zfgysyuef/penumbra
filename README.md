<img src="./docs/content/assets/banner.svg" alt="Penumbra banner">

---

Penumbra is a Rust crate and tool for interacting with Mediatek devices.<br>
It provides flashing and readback capabilities, as well as bootloader unlocking and relocking on vulnerable devices.<br>

## Requirements

* On Windows, you'll need to install MediaTek VCOM drivers. For using `linecode` exploit (also known as Kamakiri2), you'll need to install either `libusb` or `WinUSB` drivers with [Zadig](https://zadig.akeo.ie/).
* On Linux you'll need to install `libudev` and add your user to the `dialout` group. In case Penumbra doesn't recognize the device, run with sudo or allow access to the device with udev rules.

## Usage

Penumbra can be used both as a crate for interacting directly with a device with your own code, as well as providing a CLI and (preliminary) [TUI](tui).

For using the CLI, [read the documentation with all commands here](https://penumbra.itssho.my/Penumbra/Antumbra/CLI)

For using the crate, use the device API:

```rs
use std::fs::File;
use std::io::{BufWriter, Write};

use anyhow::Result;
use penumbra::{DeviceBuilder, find_mtk_port, LockFlag};

fn main() -> Result<()> {
    env_logger::init();

    let da_path = std::path::Path::new("../DA_penangf.bin");
    let da_data = std::fs::read(da_path).expect("Failed to read DA file");

    let vid = Some(0x0E8D);
    let pid = Some(0x2000);
    
    let mtk_port = PortType::find_device(vid, pid, PortBackend::Auto)
        .expect("Port should open")
        .ok_or("No MTK port found")?;

    println!("Found MTK port: {}", mtk_port.get_port_name());

    let mut device = DeviceBuilder::new(mtk_port)
        .with_da_data(da_data)
        .build()?;

    // Init the device (Handshake and populate dev info)
    device.init()?;

    let tgt_cfg = device.dev_info.target_config();
    println!("SBC: {}", (tgt_cfg & 0x1) != 0);

    // This will automatically enter DA mode. Seccfg unlock only works if the device can load extensions / is vulnerable
    device.set_seccfg_lock_state(LockFlag::Unlock)?;

    // Ignore progress for now
    let mut progress = |read: u64, total: u64| {
        println!("Progress: {}/{}", read, total);
    };

    let file = File::create("lk_a.bin")?;
    let mut writer = BufWriter::new(file);

    device.read_partition("lk_a", &mut progress, &mut writer)?;

    writer.flush()?;

    Ok(())
}
```

### Debug logs

Some issues may be hard to reproduce, and may require more insight of what is happening on the device.
If so, you can open an issue attaching debug logs.<br>
To get debug logs, run `antumbra` with the `-v` and `-l debug` flags. A file called `antumbra.log` will be created in the current directory.
This will also enable UART debug logging. If possible, attach UART logs too.
If you don't have UART, you can use the `--usb-log` flag in `antumbra` to enable DA logging over USB.
A file called `da.log` will be created in the current directory with the logs.

> [!NOTE]
> Penumbra currently supports both V5 (XFlash) and V6 (XML) devices. Issues reporting incompatibility with older (V3/Legacy) chipsets will be ignored until broader support is added.
> If your device falls in one of the supported protocols and you get the "unknown hardware code" warning, please open an issue attaching your device info, and relevant firmware
> files (preloader, DA, lk).

## Contributing

For contributing, you'll first need to setup a development environment.

Read on how to setup a dev environment and how to get started [here](CONTRIBUTING.md)

For contributing to the payloads, head to the [payloads repository](https://github.com/shomykohai/mtk-payloads).

### Current Roadmap

Core:
* [ ] Add V3 support
* [ ] Add amonet exploit

TUI:
* [x] Refactor the TUI code to be more maintainable
* [x] Add reusable components
* [x] Make better key bindings

CLI:
* [ ] Add plstage
* [x] Add Read Offset, Write Offset and Erase Offset commands
* [x] Add register read/write commands

Documentation:
* [x] Add documentation for the crate
* [ ] Add linecode exploit documentation

## Learning Resources

Penumbra has [its own documentation](https://penumbra.itssho.my/), where you can learn more about Mediatek devices and how the Download protocol works.

Other learning resources I suggest are the following
* [mtkclient](https://github.com/bkerler/mtkclient)
* [moto-experiments](https://github.com/R0rt1z2/moto-experiments)
* [kaeru](https://github.com/R0rt1z2/kaeru)
* [Carbonara exploit](https://penumbra.itssho.my/Mediatek/Exploits/Carbonara)
* [mtk-payloads](https://github.com/shomykohai/mtk-payloads)
* [da-boot](https://github.com/mt6572-mainline/da-boot)
* [fenrir](https://github.com/R0rt1z2/fenrir)
* [sprig](https://github.com/R0rt1z2/sprig)
* [HeapB8 exploit technical writeup](https://blog.r0rt1z2.com/posts/exploiting-mediatek-datwo/)
* [hacc](https://github.com/shomykohai/hacc)

## Credits

* [ChimeraTool team](https://chimeratool.com/) - heapb8 was originally reverse-engineered from ChimeraTool.

## License

Penumbra is licensed under the GNU Affero General Public License v3 or later (AGPL-3.0-or-later), see [LICENSE](LICENSE) for details.

Logo by [@archaeopteryz](https://github.com/archaeopteryz), all rights reserved. Use is allowed only for referencing "Penumbra" or "Antumbra", unless explicit permission has been granted.
