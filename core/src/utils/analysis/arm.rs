/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/

use super::ArchAnalyzer;
use crate::utils::analysis::Arch;

pub struct ArmAnalyzer {
    data: Box<[u8]>,
    base_addr: u64,
}

impl ArmAnalyzer {
    pub const fn new(data: Box<[u8]>, base_addr: u64) -> Self {
        Self { data, base_addr }
    }

    /// Decodes MOVW instruction.
    /// Returns (register, imm16)
    pub const fn decode_movw(&self, instr: u32) -> Option<(u8, u32)> {
        // MOVW: cond 0011 0000 imm4 Rd imm12
        // Encoding: 0xE30xxxxx
        if (instr & 0x0FF00000) != 0x03000000 {
            return None;
        }

        let rd = ((instr >> 12) & 0xF) as u8;
        let imm4 = (instr >> 16) & 0xF;
        let imm12 = instr & 0xFFF;
        let imm16 = (imm4 << 12) | imm12;

        Some((rd, imm16))
    }

    /// Decodes MOVT instruction.
    /// Returns (register, imm16)
    pub const fn decode_movt(&self, instr: u32) -> Option<(u8, u32)> {
        // MOVT: cond 0011 0100 imm4 Rd imm12
        // Encoding: 0xE34xxxxx
        if (instr & 0x0FF00000) != 0x03400000 {
            return None;
        }

        let rd = ((instr >> 12) & 0xF) as u8;
        let imm4 = (instr >> 16) & 0xF;
        let imm12 = instr & 0xFFF;
        let imm16 = (imm4 << 12) | imm12;

        Some((rd, imm16))
    }

    /// Decodes SUB register instruction.
    /// SUB Rd, Rn, Rm
    /// Returns (Rn, Rm, Rd)
    pub const fn decode_sub_reg(&self, instr: u32) -> Option<(u8, u8, u8)> {
        // SUB (register): cond 0000 010S nnnn dddd 0000 0000 mmmm
        if (instr & 0x0FE00FF0) != 0x00400000 {
            return None;
        }

        let rn = ((instr >> 16) & 0xF) as u8;
        let rd = ((instr >> 12) & 0xF) as u8;
        let rm = (instr & 0xF) as u8;

        Some((rn, rm, rd))
    }

    /// Checks if instruction is BX LR (return)
    pub const fn is_bx_lr(&self, instr: u32) -> bool {
        // BX LR: 0xE12FFF1E
        (instr & 0x0FFFFFFF) == 0x012FFF1E
    }

    const fn decode_ldr_pc(&self, instr: u32, pc: u64) -> Option<(u8, u64)> {
        if (instr & 0x0C5F0000) != 0x041F0000 {
            return None;
        }

        let u_bit = (instr >> 23) & 1;
        let rd = ((instr >> 12) & 0xF) as u8;
        let imm12 = instr & 0xFFF;

        let arm_pc = pc + 8;
        let target_addr = if u_bit == 1 {
            arm_pc.wrapping_add(imm12 as u64)
        } else {
            arm_pc.wrapping_sub(imm12 as u64)
        };

        Some((rd, target_addr))
    }

    const fn decode_bl(&self, instr: u32, pc: u64) -> Option<u64> {
        let opcode = instr & 0x0F000000;

        if opcode != 0x0A000000 && opcode != 0x0B000000 {
            return None;
        }

        let mut offset = (instr & 0x00FFFFFF) as i32;
        if offset & 0x00800000 != 0 {
            offset |= !0x00FFFFFF;
        }

        let arm_pc = pc + 8;
        Some(arm_pc.wrapping_add((offset * 4) as u64))
    }

    const fn decode_mov(&self, instr: u32) -> Option<(u8, u8)> {
        if (instr & 0x0FE00FF0) == 0x01A00000 {
            let rd = ((instr >> 12) & 0xF) as u8;
            let rm = (instr & 0xF) as u8;
            return Some((rm, rd));
        }
        None
    }

    const fn is_prologue(&self, instr: u32) -> bool {
        // PUSH with LR: STMDB SP!, {..., LR}
        if (instr & 0xFFFF0000) == 0xE92D0000 && (instr & (1 << 14)) != 0 {
            return true;
        }
        false
    }

    fn find_string(&self, target: &str) -> Option<usize> {
        let target_bytes = target.as_bytes();
        let mut with_null = target_bytes.to_vec();
        with_null.push(0);

        if let Some(pos) =
            self.data.windows(with_null.len()).position(|window| window == with_null.as_slice())
        {
            return Some(pos);
        }
        self.data.windows(target_bytes.len()).position(|window| window == target_bytes)
    }

    fn find_string_xref_inner(&self, target_str: &str) -> Option<usize> {
        let str_off = self.find_string(target_str)?;
        let str_va = (self.base_addr + str_off as u64) as u32;

        let low16 = (str_va & 0xFFFF) as u16;
        let high16 = (str_va >> 16) as u16;

        let len = self.data.len();

        for offset in (0..len.saturating_sub(8)).step_by(4) {
            let instr1 = self.read_u32(offset)?;

            if !self.is_movw_imm(instr1, low16) {
                continue;
            }

            let reg = self.get_movw_reg(instr1);

            let end = (offset + 20 * 4).min(len);
            for lookahead_offset in (offset + 4..end).step_by(4) {
                let instr2 = self.read_u32(lookahead_offset)?;

                if self.is_movt_imm(instr2, high16) && self.get_movt_reg(instr2) == reg {
                    return Some(offset);
                }
            }
        }

        for offset in (0..len.saturating_sub(4)).step_by(4) {
            let instr = self.read_u32(offset)?;
            let pc = self.base_addr + offset as u64;

            if let Some((_, addr)) = self.decode_ldr_pc(instr, pc)
                && addr == str_va as u64
            {
                return Some(offset);
            }
        }

        None
    }

    const fn is_movw_imm(&self, instr: u32, imm16: u16) -> bool {
        if (instr & 0x0FF00000) != 0x03000000 {
            return false;
        }

        let imm4 = (instr >> 16) & 0xF;
        let imm12 = instr & 0xFFF;
        let decoded_imm16 = (imm4 << 12) | imm12;

        decoded_imm16 == imm16 as u32
    }

    const fn is_movt_imm(&self, instr: u32, imm16: u16) -> bool {
        if (instr & 0x0FF00000) != 0x03400000 {
            return false;
        }

        let imm4 = (instr >> 16) & 0xF;
        let imm12 = instr & 0xFFF;
        let decoded_imm16 = (imm4 << 12) | imm12;

        decoded_imm16 == imm16 as u32
    }

    const fn get_movw_reg(&self, instr: u32) -> u8 {
        ((instr >> 12) & 0xF) as u8
    }

    const fn get_movt_reg(&self, instr: u32) -> u8 {
        ((instr >> 12) & 0xF) as u8
    }

    fn find_function_start(&self, from_offset: usize) -> Option<usize> {
        const LIMIT: usize = 0x2000;
        let end = from_offset.saturating_sub(LIMIT);
        let mut current = from_offset;

        while current >= end && current > 0 {
            if let Some(instr) = self.read_u32(current)
                && self.is_prologue(instr)
            {
                return Some(current);
            }

            if current < 4 {
                break;
            }
            current -= 4;
        }
        None
    }

    fn resolve_reg_recursive(
        &self,
        start_off: usize,
        end_limit: usize,
        target_reg: u8,
        depth: usize,
    ) -> Option<u64> {
        if depth > 10 || start_off < end_limit {
            return None;
        }

        let mut current_reg = target_reg;
        let mut known_high_16: Option<u32> = None;
        let mut off = start_off;

        while off >= end_limit {
            let Some(instr) = self.read_u32(off) else { break };
            let pc = self.base_addr + off as u64;

            if let Some((rd, imm16)) = self.decode_movt(instr)
                && rd == current_reg
            {
                known_high_16 = Some(imm16 << 16);
                if off < 4 {
                    break;
                }
                off -= 4;
                continue;
            }

            if let Some((rd, imm16)) = self.decode_movw(instr)
                && rd == current_reg
            {
                let mut val = imm16 as u64;
                if let Some(high) = known_high_16 {
                    val |= high as u64;
                }
                return Some(val);
            }

            if let Some((rn, rm, rd)) = self.decode_sub_reg(instr)
                && rd == current_reg
            {
                if off < 4 {
                    return None;
                }
                let prev_off = off - 4;

                let val_n = self.resolve_reg_recursive(prev_off, end_limit, rn, depth + 1)?;
                let val_m = self.resolve_reg_recursive(prev_off, end_limit, rm, depth + 1)?;

                let mut val = val_n.wrapping_sub(val_m);

                if let Some(high) = known_high_16 {
                    val = (val & 0xFFFF) | (high as u64);
                }
                return Some(val);
            }

            if let Some((rd, addr)) = self.decode_ldr_pc(instr, pc)
                && rd == current_reg
            {
                let pool_off = self.va_to_off(addr)?;
                let mut val = self.read_u32(pool_off)? as u64;

                if let Some(high) = known_high_16 {
                    val = (val & 0xFFFF) | (high as u64);
                }
                return Some(val);
            }

            if let Some((rm, rd)) = self.decode_mov(instr)
                && rd == current_reg
            {
                current_reg = rm;
            }

            if off < 4 {
                break;
            }
            off -= 4;
        }

        None
    }
}

impl ArchAnalyzer for ArmAnalyzer {
    fn va_to_off(&self, va: u64) -> Option<usize> {
        if va < self.base_addr {
            return None;
        }
        let offset = (va - self.base_addr) as usize;
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
        let len = self.data.len();

        for off in (xref..len).step_by(4) {
            let instr = self.read_u32(off)?;

            if self.decode_bl(instr, 0).is_some() {
                return self.reg_value(off, arg_idx, 50);
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
        let len = self.data.len();

        for off in (offset..len).step_by(4) {
            let instr = self.read_u32(off)?;

            let opcode = instr & 0x0F000000;
            if opcode != 0x0B000000 {
                continue;
            }

            if self.decode_bl(instr, 0).is_some() {
                return Some(off);
            }
        }

        None
    }

    fn next_b_from_off(&self, offset: usize) -> Option<usize> {
        let len = self.data.len();

        for off in (offset..len).step_by(4) {
            let instr = self.read_u32(off)?;

            let opcode = instr & 0x0F000000;
            if opcode != 0x0A000000 {
                continue;
            }

            if self.decode_bl(instr, 0).is_some() {
                return Some(off);
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
        Arch::Arm
    }

    fn reg_value(&self, at_offset: usize, target_reg: u8, lookback: usize) -> Option<u64> {
        let start_off = at_offset.saturating_sub(4);
        let end_limit = at_offset.saturating_sub(lookback * 4);

        self.resolve_reg_recursive(start_off, end_limit, target_reg, 0)
    }
}
