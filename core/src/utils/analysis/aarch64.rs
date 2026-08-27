/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/

use memchr::memmem;

use super::ArchAnalyzer;
use crate::utils::analysis::Arch;

pub struct Aarch64Analyzer {
    data: Box<[u8]>,
    base_addr: u64,
}

impl Aarch64Analyzer {
    pub const fn new(data: Box<[u8]>, base_addr: u64) -> Self {
        Self { data, base_addr }
    }

    /// Decodes ADRP instruction.
    ///
    /// ADRP Encoding:
    /// * OPCode: 0x90000000
    /// * Rd = destination register (bits [4:0])
    /// * immlo = bits [30:29]
    /// * immhi = bits [23:5]
    ///
    /// Returns (page_address, destination_register)
    pub const fn decode_adrp(&self, instr: u32, pc: u64) -> Option<(u64, u8)> {
        if (instr & 0x9F000000) != 0x90000000 {
            return None;
        }

        let rd = (instr & 0x1F) as u8;
        let immlo = ((instr >> 29) & 0x3) as i64;
        let immhi = ((instr >> 5) & 0x7FFFF) as i64;

        let mut imm = (immhi << 2) | immlo;
        if imm & 0x100000 != 0 {
            imm -= 0x200000;
        }

        let page = pc & !0xFFF;
        Some((page.wrapping_add((imm << 12) as u64), rd))
    }

    /// Decodes ADD immediate instruction (64-bit).
    ///
    /// Encoding: 0x91000000 | (shift << 22) | (imm12 << 10) | (Rn << 5) | Rd
    ///
    /// Returns (source_register, destination_register, immediate_value)
    pub const fn decode_add_imm(&self, instr: u32) -> Option<(u8, u8, u32)> {
        if (instr & 0xFF000000) != 0x91000000 {
            return None;
        }

        let rn = ((instr >> 5) & 0x1F) as u8;
        let rd = (instr & 0x1F) as u8;
        let imm12 = (instr >> 10) & 0xFFF;
        let shift = (instr >> 22) & 1;

        let imm = if shift != 0 { imm12 << 12 } else { imm12 };

        Some((rn, rd, imm))
    }

    pub const fn is_pointer_auth(&self, instr: u32) -> bool {
        instr == 0xD503233F || instr == 0xD503237F || instr == 0xD50303BF
    }

    /// Decodes BL instruction.
    ///
    /// Returns the target address.
    const fn decode_bl(&self, instr: u32, pc: u64) -> Option<u64> {
        let opcode = instr & 0xFC000000;

        if opcode != 0x94000000 && opcode != 0x14000000 {
            return None;
        }

        let mut off = (instr & 0x03FFFFFF) as i32;
        if off & 0x02000000 != 0 {
            off -= 0x04000000;
        }

        Some(pc.wrapping_add((off * 4) as u64))
    }

    /// Decodes MOV register instruction (ORR with XZR).
    ///
    /// Returns (source_register, destination_register)
    const fn decode_mov_register(&self, instr: u32) -> Option<(u8, u8)> {
        if (instr & 0xFFE0FFE0) != 0xAA0003E0 {
            return None;
        }

        Some((((instr >> 16) & 0x1F) as u8, (instr & 0x1F) as u8))
    }

    fn find_string(&self, s: &str) -> Option<usize> {
        memmem::find(&self.data, s.as_bytes())
    }

    fn find_string_xref_inner(&self, s: &str) -> Option<usize> {
        let off = self.find_string(s)?;
        let va = self.base_addr + off as u64;
        let page = va & !0xFFF;

        for off in (0..self.data.len()).step_by(4) {
            let instr = self.read_u32(off)?;
            let pc = self.base_addr + off as u64;

            let (adrp_page, reg) = match self.decode_adrp(instr, pc) {
                Some(v) => v,
                None => continue,
            };

            if adrp_page != page {
                continue;
            }

            if let Some(add_off) = self.find_matching_add(off, reg, va) {
                return Some(add_off);
            }
        }
        None
    }

    fn find_matching_add(&self, base: usize, reg: u8, va: u64) -> Option<usize> {
        for off in (base + 4..base + 64).step_by(4) {
            let instr = self.read_u32(off)?;
            let (rn, _rd, imm) = match self.decode_add_imm(instr) {
                Some(decoded) => decoded,
                None => continue,
            };

            if rn == reg && (va & !0xFFF) + imm as u64 == va {
                return Some(off);
            }
        }
        None
    }

    fn find_function_start(&self, from: usize) -> Option<usize> {
        const MASK: u32 = 0xFFC07FFF;
        const PATTERN: u32 = 0xA9807BFD;

        const MASK_FALLBACK: u32 = 0xFFC003FF;
        const PATTERN_FALLBACK: u32 = 0xA9007BFD;

        const LIMIT: usize = 0x5000;
        let end = from.saturating_sub(LIMIT);

        for off in (end..=from).rev().step_by(4) {
            let instr = self.read_u32(off)?;
            if off >= 4 {
                let prev = self.read_u32(off - 4)?;

                // Pointer authentication
                if prev == 0xD503233F || prev == 0xD503237F {
                    return Some(off - 4);
                }

                // Sub SP before STP
                if (prev & 0xFFC003FF) == 0xD10003FF
                    && (instr & MASK_FALLBACK) == (PATTERN_FALLBACK & MASK_FALLBACK)
                {
                    return Some(off - 4);
                }
            }

            if (instr & MASK) == PATTERN {
                return Some(off);
            }
        }

        None
    }

    /// Resolves the ADRP part of an ADD instruction by scanning backwards.
    fn resolve_adrp_part(&self, from: usize, limit: usize, reg: u8, imm: u32) -> Option<u64> {
        if from < limit {
            return None;
        }

        let mut off = from;

        loop {
            let instr = self.read_u32(off)?;
            let pc = self.base_addr + off as u64;

            if let Some((page, rd)) = self.decode_adrp(instr, pc)
                && rd == reg
            {
                return Some(page + imm as u64);
            }

            if off <= limit || off < 4 {
                break;
            }
            off -= 4;
        }

        None
    }
}

impl ArchAnalyzer for Aarch64Analyzer {
    fn va_to_off(&self, va: u64) -> Option<usize> {
        va.checked_sub(self.base_addr)
            .and_then(|o| usize::try_from(o).ok())
            .filter(|&o| o < self.data.len())
    }

    fn off_to_va(&self, offset: usize) -> Option<u64> {
        if offset < self.data.len() { Some(self.base_addr + offset as u64) } else { None }
    }

    fn fn_from_str(&self, s: &str) -> Option<usize> {
        let xref = self.find_string_xref_inner(s)?;
        self.find_function_start(xref)
    }

    fn find_call_arg_from_string(&self, s: &str, arg: u8) -> Option<u64> {
        let start = self.find_string_xref_inner(s)?;

        for off in (start..self.data.len()).step_by(4) {
            let instr = self.read_u32(off)?;
            if (instr & 0xFC000000) == 0x94000000 {
                return self.reg_value(off, arg, 50);
            }
        }

        None
    }

    fn bl_target(&self, offset: usize) -> Option<u64> {
        let instr = self.read_u32(offset)?;
        let pc = self.off_to_va(offset)?;
        self.decode_bl(instr, pc)
    }

    fn b_target(&self, offset: usize) -> Option<u64> {
        self.bl_target(offset)
    }

    fn next_bl_from_off(&self, offset: usize) -> Option<usize> {
        for off in (offset..self.data.len()).step_by(4) {
            let instr = self.read_u32(off)?;
            if (instr & 0xFC000000) == 0x94000000 {
                return Some(off);
            }
        }
        None
    }

    fn next_b_from_off(&self, offset: usize) -> Option<usize> {
        for off in (offset..self.data.len()).step_by(4) {
            let instr = self.read_u32(off)?;
            if (instr & 0xFC000000) == 0x14000000 {
                return Some(off);
            }
        }
        None
    }

    fn str_xref(&self, s: &str) -> Option<usize> {
        self.find_string_xref_inner(s)
    }

    fn fn_from_off(&self, offset: usize) -> Option<usize> {
        self.find_function_start(offset)
    }

    fn data(&self) -> &[u8] {
        &self.data
    }

    fn arch(&self) -> Arch {
        Arch::Aarch64
    }

    /// Resolves the value of a register at a given offset by scanning backwards.
    fn reg_value(&self, at: usize, reg: u8, lookback: usize) -> Option<u64> {
        let start = at.saturating_sub(lookback * 4);
        let mut cur_reg = reg;

        for off in (start..=at).rev().step_by(4) {
            let instr = self.read_u32(off)?;

            if let Some((rn, rd, imm)) = self.decode_add_imm(instr)
                && rd == cur_reg
            {
                return self.resolve_adrp_part(off, start, rn, imm);
            }

            if let Some((rm, rd)) = self.decode_mov_register(instr)
                && rd == cur_reg
            {
                cur_reg = rm;
            }
        }

        None
    }
}
