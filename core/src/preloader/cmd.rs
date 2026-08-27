/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/

/// Commands used by the Preloader / BROM protocol
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Command {
    StayStill = 0x80,
    SendDebugAuth = 0x88,

    LegacyWrite = 0xA1,
    LegacyRead = 0xA2,

    I2cInit = 0xB0,
    I2cDeinit = 0xB1,
    I2cWrite8 = 0xB2,
    I2cRead8 = 0xB3,
    I2cSetSpeed = 0xB4,

    PwrInit = 0xC4,
    PwrDeinit = 0xC5,
    PwrRead16 = 0xC6,
    PwrWrite16 = 0xC7,
    CacheControl = 0xC8,

    Read16 = 0xD0,
    Read32 = 0xD1,
    Write16 = 0xD2,
    Write16NoEcho = 0xD3,
    Write32 = 0xD4,
    JumpDa = 0xD5,
    JumpBl = 0xD6,
    SendDa = 0xD7,
    GetTargetConfig = 0xD8,
    SendEppParam = 0xD9,
    SysRegionAccess = 0xDA,
    Uart1LogEn = 0xDB,
    Uart1SetBaudrate = 0xDC,
    GetBromLog = 0xDD,
    JumpDa64 = 0xDE,
    GetBromLogWithStatus = 0xDF,

    SendCert = 0xE0,
    GetMeId = 0xE1,
    SendAuth = 0xE2,
    SlaChallenge = 0xE3,
    GenExpirationId = 0xE4,
    SendRootCert = 0xE5,
    GetRootCertInfo = 0xE6,
    GetSocId = 0xE7,
    SendCertWithStatus = 0xE8,

    Zeroization = 0xF0,
    GetPlCap = 0xFB,
    GetHwSwVer = 0xFC,
    GetHwCode = 0xFD,
    GetPlVer = 0xFE,
    GetBrVer = 0xFF,
}
