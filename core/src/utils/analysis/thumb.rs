/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/

use super::ArchAnalyzer;
use crate::utils::analysis::Arch;

pub struct Thumb2Analyzer {
    data: Box<[u8]>,
    base_addr: u64,
}

impl Thumb2Analyzer {
    pub const fn new(data: Box<[u8]>, base_addr: u64) -> Self {
        Self { data, base_addr }
    }

    fn read_u16(&self, offset: usize) -> Option<u16> {
        if offset + 1 >= self.data.len() {
            return None;
        }
        Some(u16::from_le_bytes([self.data[offset], self.data[offset + 1]]))
    }

    fn read_thumb32(&self, offset: usize) -> Option<u32> {
        let hw1 = self.read_u16(offset)? as u32;
        let hw2 = self.read_u16(offset + 2)? as u32;
        Some((hw1 << 16) | hw2)
    }

    fn is_wide_instruction(&self, offset: usize) -> bool {
        self.read_u16(offset).is_some_and(|hw| {
            let top5 = hw >> 11;
            top5 == 0b11101 || top5 == 0b11110 || top5 == 0b11111
        })
    }

    fn instruction_size(&self, offset: usize) -> usize {
        if self.is_wide_instruction(offset) { 4 } else { 2 }
    }

    pub const fn decode_movw(&self, instr: u32) -> Option<(u8, u32)> {
        if (instr & 0xFBF08000) != 0xF2400000 {
            return None;
        }

        let imm4 = (instr >> 16) & 0xF;
        let i = (instr >> 26) & 1;
        let imm3 = (instr >> 12) & 0x7;
        let rd = ((instr >> 8) & 0xF) as u8;
        let imm8 = instr & 0xFF;

        let imm16 = (imm4 << 12) | (i << 11) | (imm3 << 8) | imm8;

        Some((rd, imm16))
    }

    pub const fn decode_movt(&self, instr: u32) -> Option<(u8, u32)> {
        if (instr & 0xFBF08000) != 0xF2C00000 {
            return None;
        }

        let imm4 = (instr >> 16) & 0xF;
        let i = (instr >> 26) & 1;
        let imm3 = (instr >> 12) & 0x7;
        let rd = ((instr >> 8) & 0xF) as u8;
        let imm8 = instr & 0xFF;

        let imm16 = (imm4 << 12) | (i << 11) | (imm3 << 8) | imm8;

        Some((rd, imm16))
    }

    pub const fn decode_sub_reg(&self, instr: u32) -> Option<(u8, u8, u8)> {
        if (instr & 0xFFE0F0F0) != 0xEBA00000 {
            return None;
        }

        let rn = ((instr >> 16) & 0xF) as u8;
        let rd = ((instr >> 8) & 0xF) as u8;
        let rm = (instr & 0xF) as u8;

        Some((rn, rm, rd))
    }

    pub fn is_bx_lr(&self, offset: usize) -> bool {
        self.read_u16(offset) == Some(0x4770)
    }

    const fn decode_ldr_pc(&self, instr: u32, pc: u64) -> Option<(u8, u64)> {
        if (instr & 0xFF7F0000) != 0xF85F0000 {
            return None;
        }

        let u_bit = (instr >> 23) & 1;
        let rt = ((instr >> 12) & 0xF) as u8;
        let imm12 = instr & 0xFFF;

        let align_pc = (pc + 4) & !3;
        let target = if u_bit == 1 {
            align_pc.wrapping_add(imm12 as u64)
        } else {
            align_pc.wrapping_sub(imm12 as u64)
        };

        Some((rt, target))
    }

    const fn decode_bl(&self, instr: u32, pc: u64) -> Option<u64> {
        let hw1 = instr >> 16;
        let hw2 = instr & 0xFFFF;

        if (hw1 >> 11) != 0b11110 {
            return None;
        }

        if (hw2 & 0xC000) != 0xC000 {
            return None;
        }

        let is_bl = (hw2 & 0x1000) != 0;
        let is_b = !is_bl;

        if !is_bl && !is_b {
            return None;
        }

        let s = (hw1 >> 10) & 1;
        let imm10 = hw1 & 0x3FF;
        let j1 = (hw2 >> 13) & 1;
        let j2 = (hw2 >> 11) & 1;
        let imm11 = hw2 & 0x7FF;

        let i1 = !(j1 ^ s) & 1;
        let i2 = !(j2 ^ s) & 1;

        let mut offset = (s << 24) | (i1 << 23) | (i2 << 22) | (imm10 << 12) | (imm11 << 1);

        if s != 0 {
            offset |= 0xFE000000;
        }

        let offset = offset as i32;
        // In ARM, PC = current instruction + 4
        Some((pc + 4).wrapping_add(offset as u64))
    }

    const fn decode_mov(&self, hw: u16) -> Option<(u8, u8)> {
        if (hw & 0xFF00) != 0x4600 {
            return None;
        }

        let d = ((hw >> 7) & 1) as u8;
        let rm = ((hw >> 3) & 0xF) as u8;
        let rd = (hw as u8 & 0x7) | (d << 3);

        Some((rm, rd))
    }

    fn is_prologue(&self, offset: usize) -> bool {
        if let Some(instr) = self.read_thumb32(offset) {
            let hw1 = instr >> 16;
            if hw1 == 0xE92D && (instr & (1 << 14)) != 0 {
                return true;
            }
        }

        if let Some(hw) = self.read_u16(offset)
            && (hw & 0xFF00) == 0xB500
        {
            return true;
        }

        false
    }

    fn find_string(&self, target: &str) -> Option<usize> {
        let target_bytes = target.as_bytes();
        let mut with_null = target_bytes.to_vec();
        with_null.push(0);

        if let Some(pos) =
            self.data.windows(with_null.len()).position(|w| w == with_null.as_slice())
        {
            return Some(pos);
        }
        self.data.windows(target_bytes.len()).position(|w| w == target_bytes)
    }

    fn find_string_xref_inner(&self, target_str: &str) -> Option<usize> {
        let str_off = self.find_string(target_str)?;
        let str_va = (self.base_addr + str_off as u64) as u32;

        let low16 = (str_va & 0xFFFF) as u16;
        let high16 = (str_va >> 16) as u16;

        let len = self.data.len();

        // movw + movt
        let mut offset = 0;
        while offset + 8 < len {
            if !self.is_wide_instruction(offset) {
                offset += 2;
                continue;
            }

            let instr1 = match self.read_thumb32(offset) {
                Some(v) => v,
                None => break,
            };

            if !self.is_movw_imm(instr1, low16) {
                offset += 4;
                continue;
            }

            let reg = self.get_movw_reg(instr1);

            let end = (offset + 20 * 4).min(len);
            let mut la = offset + 4;
            while la < end {
                let sz = self.instruction_size(la);
                if sz == 4
                    && let Some(instr2) = self.read_thumb32(la)
                    && self.is_movt_imm(instr2, high16)
                    && self.get_movt_reg(instr2) == reg
                {
                    return Some(offset);
                }
                la += sz;
            }

            offset += 4;
        }

        // LDR + PC (sane way :D)
        offset = 0;
        while offset + 4 <= len {
            if self.is_wide_instruction(offset) {
                if let Some(instr) = self.read_thumb32(offset) {
                    let pc = self.base_addr + offset as u64;
                    if let Some((_, addr)) = self.decode_ldr_pc(instr, pc) {
                        if addr == str_va as u64 {
                            return Some(offset);
                        }
                        if let Some(pool_off) = self.va_to_off(addr)
                            && let Some(val) = self.read_u32(pool_off)
                            && val as u64 == str_va as u64
                        {
                            return Some(offset);
                        }
                    }
                }
                offset += 4;
            } else {
                if let Some(hw) = self.read_u16(offset)
                    && (hw & 0xF800) == 0x4800
                {
                    let rt = ((hw >> 8) & 0x7) as u8;
                    let imm8 = (hw & 0xFF) as u64;
                    let pc = self.base_addr + offset as u64;
                    let target = ((pc + 4) & !3) + (imm8 << 2);

                    if let Some(pool_off) = self.va_to_off(target)
                        && let Some(val) = self.read_u32(pool_off)
                    {
                        let val_clean = val & !1;
                        if val_clean == str_va || val == str_va {
                            let _ = rt;
                            return Some(offset);
                        }
                    }
                }
                offset += 2;
            }
        }

        None
    }

    const fn is_movw_imm(&self, instr: u32, imm16: u16) -> bool {
        if let Some((_, decoded)) = self.decode_movw(instr) {
            decoded == imm16 as u32
        } else {
            false
        }
    }

    const fn is_movt_imm(&self, instr: u32, imm16: u16) -> bool {
        if let Some((_, decoded)) = self.decode_movt(instr) {
            decoded == imm16 as u32
        } else {
            false
        }
    }

    const fn get_movw_reg(&self, instr: u32) -> u8 {
        ((instr >> 8) & 0xF) as u8
    }

    const fn get_movt_reg(&self, instr: u32) -> u8 {
        ((instr >> 8) & 0xF) as u8
    }

    fn find_function_start(&self, from_offset: usize) -> Option<usize> {
        const LIMIT: usize = 0x2000;
        let end = from_offset.saturating_sub(LIMIT);
        let mut current = from_offset;

        while current >= end && current > 0 {
            if self.is_prologue(current) {
                return Some(current);
            }

            if current < 2 {
                break;
            }
            current -= 2;
        }
        None
    }

    const fn iter_instructions_from(&self, start: usize) -> Thumb2InstrIter<'_> {
        Thumb2InstrIter { analyzer: self, offset: start }
    }
}

struct Thumb2InstrIter<'a> {
    analyzer: &'a Thumb2Analyzer,
    offset: usize,
}

impl<'a> Iterator for Thumb2InstrIter<'a> {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.analyzer.data.len() {
            return None;
        }
        let off = self.offset;
        let sz = self.analyzer.instruction_size(off);
        if off + sz > self.analyzer.data.len() {
            return None;
        }
        self.offset += sz;
        Some((off, sz))
    }
}

impl ArchAnalyzer for Thumb2Analyzer {
    fn va_to_off(&self, va: u64) -> Option<usize> {
        let va_clean = va & !1;
        if va_clean < self.base_addr {
            return None;
        }

        let offset = (va_clean - self.base_addr) as usize;
        if offset >= self.data.len() {
            return None;
        }
        Some(offset)
    }

    fn off_to_va(&self, offset: usize) -> Option<u64> {
        if offset >= self.data.len() {
            return None;
        }

        Some(self.base_addr + offset as u64)
    }

    fn fn_from_str(&self, s: &str) -> Option<usize> {
        let xref = self.str_xref(s)?;
        self.find_function_start(xref)
    }

    fn find_call_arg_from_string(&self, s: &str, arg_idx: u8) -> Option<u64> {
        let xref = self.str_xref(s)?;

        for (off, sz) in self.iter_instructions_from(xref) {
            if sz == 4 {
                let instr = self.read_thumb32(off)?;
                if (instr & 0xF800D000) == 0xF000D000 {
                    return self.reg_value(off, arg_idx, 50);
                }
            }
        }

        None
    }

    fn bl_target(&self, offset: usize) -> Option<u64> {
        let instr = self.read_thumb32(offset)?;
        let pc = self.base_addr + offset as u64;
        self.decode_bl(instr, pc)
    }

    fn b_target(&self, offset: usize) -> Option<u64> {
        self.bl_target(offset)
    }

    fn next_bl_from_off(&self, offset: usize) -> Option<usize> {
        for (off, sz) in self.iter_instructions_from(offset) {
            if sz == 4
                && let Some(instr) = self.read_thumb32(off)
            {
                let hw2 = instr & 0xFFFF;
                if (instr >> 27) == 0b11110 && (hw2 & 0xD000) == 0xD000 {
                    return Some(off);
                }
            }
        }
        None
    }

    fn next_b_from_off(&self, offset: usize) -> Option<usize> {
        for (off, sz) in self.iter_instructions_from(offset) {
            if sz == 4
                && let Some(instr) = self.read_thumb32(off)
            {
                let hw2 = instr & 0xFFFF;
                if (instr >> 27) == 0b11110 && (hw2 & 0xD000) == 0x9000 {
                    return Some(off);
                }
            }
        }
        None
    }

    fn str_xref(&self, target_str: &str) -> Option<usize> {
        self.find_string_xref_inner(target_str)
    }

    fn fn_from_off(&self, offset: usize) -> Option<usize> {
        self.find_function_start(offset)
    }

    fn data(&self) -> &[u8] {
        &self.data
    }

    fn arch(&self) -> Arch {
        Arch::Thumb2
    }

    fn reg_value(&self, at_offset: usize, target_reg: u8, lookback: usize) -> Option<u64> {
        let start = at_offset.saturating_sub(lookback * 4);
        let mut reg = target_reg;
        let mut off = at_offset;

        while off >= start {
            let sz = if self.is_wide_instruction(off) { 4 } else { 2 };

            if sz == 4 {
                let instr = self.read_thumb32(off)?;
                let pc = self.base_addr + off as u64;

                if let Some((rd, addr)) = self.decode_ldr_pc(instr, pc)
                    && rd == reg
                {
                    let pool_off = self.va_to_off(addr)?;
                    return self.read_u32(pool_off).map(|v| v as u64);
                }

                if let Some((rd, imm)) = self.decode_movw(instr)
                    && rd == reg
                {
                    let mut la = off + 4;
                    let la_end = (off + 20 * 4).min(self.data.len());
                    while la < la_end {
                        let la_sz = self.instruction_size(la);
                        if la_sz == 4
                            && let Some(la_instr) = self.read_thumb32(la)
                            && let Some((t_rd, t_imm)) = self.decode_movt(la_instr)
                            && t_rd == reg
                        {
                            return Some(((t_imm << 16) | imm) as u64);
                        }
                        la += la_sz;
                    }

                    return Some(imm as u64);
                }
            } else if let Some(hw) = self.read_u16(off)
                && let Some((rm, rd)) = self.decode_mov(hw)
                && rd == reg
            {
                reg = rm;
            }

            if off < 2 {
                break;
            }

            off -= 2;
        }

        None
    }
}
