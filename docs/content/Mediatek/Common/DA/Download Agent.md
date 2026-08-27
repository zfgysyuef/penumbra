## General Info

In Mediatek devices, a Download Agent (DA) is a special file containg code that is sent to the device through [[BROM]] or [[Preloader]] via serial communication.

The DA allows through specialized tools (like [[SP Flash Tool]], [[mtkclient]] and [[penumbra]]) to perform a variety of specified actions on the device, most notably:
* Reading and writing flash
* Read and Write RPMB (Through [[DA Extensions]] or a a specialised DA)
* Get info on the device (Chip ID, MEID, OTP ver...)
* Read and Write e-fuses
* Unlock or relock [[Seccfg]] (Through [[DA Extensions]])

A Download Agent operates under three different protocols (ordered from oldest to newest):

* Legacy (V3)
* [[XFlash DA Protocol|XFlash (V5)]]
* [[XML DA Protocol|XML (V6)]] 


## Download Agent stages

A DA file contains two stages: DA1 and DA2.

### DA1

The first stage (DA1) is responsible to initializing the platform and environment for allowing then to boot the second stage.
DA1 (on secure chips) is bound to DAA (Download Agent Authorization) to allow to continue.
The Preloader will verify the signature of the first stage against the Public Key. If DAA doesn't pass, the first stage won't be loaded and an assertion will happen.

Exploits like [[Kamakiri]] allows to temporarily disable phone security protections (SLA, DAA and SBC) to be able to load an arbitrary patched DA1.

After the first stage is loaded, only some commands are available, most notably `cmd_boot_to`, which is used to load the second stage (DA2).
The `boot_to` cmd includes some protections, like hash and secure boot checks for allowing the incoming second stage to load.

[[Carbonara]] abuses a flawed implementation of this cmd to overwrite the DA2 hash contained in DA1, disrupting this way the whole root of trust and allowing to load an arbitrary payload.

### DA2

The second stage (DA2) is what's responsible to use the full suite of commands, such as writing, reading and formatting partitions, writing efuses, read and write RPMB (before 2024).

DA2 can optionally implement `DA SLA`, a form of authentication similar to Preloader / Brom SLA, that (if present) needs to be completed before being able to execute any command.

Before some still unknown time in 2024, DA2 also implemented the `boot_to` cmd, which allowed [[DA Extensions]] to be arbitrary loaded with a patched second stage.


## Download Agent Structure

![[da_v5.png]]
*DA V5 structure*

Between [[Legacy DA Protocol|Legacy]], [[XFlash DA Protocol|XFlash]] and [[XML DA Protocol|XML]] DA, the structure varies with small differences, but generally it's quite consistent.

| Data found           | Offset      | Description                                                                                                                                           |
| -------------------- | ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| DA File Magic String | `0x0-0x12`  | Always `MTK_DOWNLOAD_AGENT`                                                                                                                           |
| DA File ID           | `0x20-0x60` | In XFlash: `MTK_AllInOne_DA_v3`<br>In XML: `MTK_DA_v6`                                                                                                |
| DA Version           | `0x60-0x64` | Seem to always stay `4` in all DA files I've analyzed                                                                                                 |
| DA Magic             | `0x64-0x68` | Always `99886622`                                                                                                                                     |
| Number of SoC        | `0x68-0x6C` | How many DA entries are in one file. One DA File can contain multple DAs for many SoC                                                                 |
| DA Entries           | `0x6C-0x??` | Each DA entries contains metadata on the DA and their regions.<br>On Legacy, the size of each DA entry is `0xD8`, while on XFlash and XML it's `0xDC` |

Each DA Entry has this structure (offset are adjusted using `0x0` as the beginning of the DA entry)

| Data found         | Offset                                               | Description                                                                  |
| ------------------ | ---------------------------------------------------- | ---------------------------------------------------------------------------- |
| Magic              | `0x0-0x02`                                           | Seems to always be `DADA`                                                    |
| HW Code (chipset)  | `0x02-0x04`                                          | Which chipset this DA entry works on (e.g. 6867 (LE) -> 6768)                |
| HW Sub code        | `0x04-0x06`                                          | Chipset subcode (most likely to identify with revisions of the same chipset) |
| HW Version         | `0x06-0x08`                                          | Probably another Identifier for the chipset revision                         |
| Entry region index | `0x10-0x12`                                          | Seem to always be 0                                                          |
| Entry region count | `0x12-0x14`                                          | How many regions this DA Entry has                                           |
| Region table       | `0x14-0xDC` on XML and XFlash, `0x14-0xD8` on Legacy | Metadata on each region. Each region is `0x20` bytes long                    |

Finally, a region has this structure

| Data found       | Offset      | Description                                                           |
| ---------------- | ----------- | --------------------------------------------------------------------- |
| Offset           | `0x0-0x04`  | At which offset in the DA file this region starts                     |
| Length           | `0x04-0x08` | Length of this region (Signature included)                            |
| Address          | `0x08-0x0C` | Address in which this region will be sent and loaded into the device. |
| Region length    | `0x0C-0x10` | Same as length, minus signature length                                |
| Signature length | `0x10-0x14` | How many bytes the signature of this region is long                   |

For more information on how to parse a DA, I suggest looking at these resources:
* [hacc da parser](https://github.com/shomykohai/hacc/blob/main/src/da.rs), penumbra uses this parser to parse DA files.
* [penumbra parse_da python script](https://github.com/shomykohai/penumbra/blob/main/scripts/parse_da.py)

## Download Agent Security

A DA can have some security measures.
* DA SLA (Not to confuse with Preloader or BROM SLA), after the DA2 gets uploaded and executed, auth will be required to continue. The auth is an RSA key, and can be usually be found in the `SLA_Challenge.dll` file if the DA can perform actions with SP Flash Tool.
* DAA (`Download Agent Authentication`, not DA specific, but needed for booting the DA), which verifies DA1 signature against the public key stored in the device efuses.
