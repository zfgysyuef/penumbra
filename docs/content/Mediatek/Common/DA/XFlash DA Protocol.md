## General Info

The XFlash DA Protocol (also known as V5 DA Protocol) is the communication protocol used by [[Download Agent|Download Agents]] in MediaTek devices to interact with the device during Download Mode.
It is an evolution of the older Legacy DA Protocol (which seem to be labeled as V3), which will also be the foundation for the later [[XML DA Protocol]] (V6).

## General Characteristics

The XFlash DA Protocol is found on most MediaTek devices released between 2016 and ~2022.

The communication happens over either UART (less common) or USB (most common).
The tool to use this protocol is SP Flash Tool V5.

## Communication layer

The XFlash DA Protocol (compared to Legacy), uses a more robust communication layer, where every packet is sent with an header.

The header is a 12 bytes structure with the following format:
* Magic (4 bytes): Always `0xFEEEEEEF`
* Data Type (4 bytes): Indicates the type of data being sent. Either Protocol Flow (1) or Message (2). 99% of the time it's Protocol Flow (1).
* Data Length (4 bytes): Length of the data being sent, in bytes.

After the header, the actual data is sent.

When sending a packet to the device, the host will send an header followed by the data, the same way the device does. The device will read up to the Data Length specified in the header, and will ignore any extra data.

## Commands

The XFlash protocol divides commands in two categories: Major Commands and Device Controls.
A command is identified by a u32 ID.

### Major Commands

Major Commands are the main commands used to interact with the device, and are mainly used to perform actions on the storage (flash, read, erase...).
Major commands are in the 0x0001XXXX range.

A special major command is the `DeviceCtrl` command (0x010009), which is used to use operate with the second set of commands.

### Device Controls

These commands are usually "getters" and "setters" for device parameters, like Emmc info, setting checksum level, and generally to perform operations that are not directly related to storage operations.

Device Control are divided in ranges:
* 0x020000 - 0x02FFFF: Setters (SetChecksumLevel, SetRemoteSecPolicy...)
* 0x040000 - 0x04FFFF: Getters (GetEmmcInfo, GetUsbSpeed, GetChipId...)
* 0x080001 - 0x08FFFF: Actions (StartDlInfo, EndDlInfo...)
* 0x0E0000 - 0x0EFFFF: Devices Control (CtrlStorageTest, DeviceCtrlReadRegister...)

To these, with [[DA Extensions]], a new range was added:
* 0x0F0000 - 0x0FFFFF: Extensions Controls (ReadRPMB, WriteRPMB, ReadRegister, WriteRegister, Sej...)

## Flow

### Command flow

The general flow of communication using XFlash DA Protocol is as follows:
1. The host sends a command as a u32 LE value, preceded by the header.
2. The device responds with a status code (u32), 0x0 for success, other values for errors.
3. The device enters the command, and the host and device exchange data as needed.
4. After the command returns, the device sends a final status code (u32) indicating the result of the command.
5. The device is now ready for the next command.

For device controls, the flow is similar:
1. The host sends the DeviceCtrl command (0x010009) as a u32 LE value, preceded by the header.
2. The device responds with a status code (u32), 0x0 for success
3. The host sends the specific Device Control command ID
4. The device responds with a status code (u32), 0x0 for success
5. The host and device exchange data as needed.
6. After the command returns, the device sends a final status code (u32)
7. The device is now ready for the next command.

### Download flow

During the download process, the host and device will exchange data in chunks.
To make sure the data is correctly received, the device and host will exchanges acknoledgements (0u32) after each chunk.

### Progress report

During operation that might take some time (like erasing a partition), the device will enter a progress report mode, where it will periodically send progress updates to the host.

The device will send `0x40040004` followed by a progress percentage (u32, 0-100), indicating the operation is still ongoing.

When the operation is complete, the device will send `0x40040005` and the final status code (u32).

## Error codes

The XFlash protocol introduced a more robust error code system, with each code giving us information about what it means and the domain it belongs to.

The error codes are divided in 4 severity levels:
* Success (0x00000000)
* Info (0x40000000)
* Warning (0x80000000)
* Error (0xC0000000)

Then, follows the "domain" of this error code, which indicates which component the error relates to:
* Common (1)
* Security (2)
* Library (3)
* Device/HW (4)
* Host? (5)
* BROM (6)
* DA (7)
* Preloader (8)

Finally, the actual error code (0x01-...) is appended.

Example:
`0xc0070004` => `0xC0000000` (Error) | `7 << 16` (domain) | `0x4` (code)
