/// Opcode dispatch table — replaces the multi-match enum approach with
/// a flat [fn; 256] lookup. Each handler knows its addressing mode
/// statically, eliminating decode → resolve_addr → resolve → execute
/// match chains.

use crate::cpu::{Cpu, Memory, CARRY, ZERO, NEGATIVE, OVERFLOW, DECIMAL, INTERRUPT, Status};

/// Handler signature: takes cpu + memory, returns cycles consumed.
type Handler<M> = fn(&mut Cpu, &mut M) -> u8;

// ── Addressing mode helpers ─────────────────────────────────────────

#[inline(always)]
fn read_u8<M: Memory>(cpu: &Cpu, mem: &mut M) -> u8 {
    mem.read(cpu.pc.wrapping_add(1))
}

#[inline(always)]
fn read_u16<M: Memory>(cpu: &Cpu, mem: &mut M) -> u16 {
    let lo = mem.read(cpu.pc.wrapping_add(1)) as u16;
    let hi = mem.read(cpu.pc.wrapping_add(2)) as u16;
    (hi << 8) | lo
}

#[inline(always)]
fn addr_zp<M: Memory>(cpu: &Cpu, mem: &mut M) -> u16 {
    read_u8(cpu, mem) as u16
}

#[inline(always)]
fn addr_zpx<M: Memory>(cpu: &Cpu, mem: &mut M) -> u16 {
    read_u8(cpu, mem).wrapping_add(cpu.x) as u16
}

#[inline(always)]
fn addr_zpy<M: Memory>(cpu: &Cpu, mem: &mut M) -> u16 {
    read_u8(cpu, mem).wrapping_add(cpu.y) as u16
}

#[inline(always)]
fn addr_abs<M: Memory>(cpu: &Cpu, mem: &mut M) -> u16 {
    read_u16(cpu, mem)
}

#[inline(always)]
fn addr_abx<M: Memory>(cpu: &Cpu, mem: &mut M) -> u16 {
    read_u16(cpu, mem).wrapping_add(cpu.x as u16)
}

#[inline(always)]
fn addr_aby<M: Memory>(cpu: &Cpu, mem: &mut M) -> u16 {
    read_u16(cpu, mem).wrapping_add(cpu.y as u16)
}

#[inline(always)]
fn addr_izx<M: Memory>(cpu: &Cpu, mem: &mut M) -> u16 {
    let ptr = read_u8(cpu, mem).wrapping_add(cpu.x);
    let lo = mem.read(ptr as u16) as u16;
    let hi = mem.read(ptr.wrapping_add(1) as u16) as u16;
    (hi << 8) | lo
}

#[inline(always)]
fn addr_izy<M: Memory>(cpu: &Cpu, mem: &mut M) -> u16 {
    let ptr = read_u8(cpu, mem);
    let lo = mem.read(ptr as u16) as u16;
    let hi = mem.read(ptr.wrapping_add(1) as u16) as u16;
    ((hi << 8) | lo).wrapping_add(cpu.y as u16)
}

// Page cross detection
#[inline(always)]
fn page_cross_abx<M: Memory>(cpu: &Cpu, mem: &mut M) -> u8 {
    let base = read_u16(cpu, mem);
    if (base & 0xFF00) != (base.wrapping_add(cpu.x as u16) & 0xFF00) { 1 } else { 0 }
}

#[inline(always)]
fn page_cross_aby<M: Memory>(cpu: &Cpu, mem: &mut M) -> u8 {
    let base = read_u16(cpu, mem);
    if (base & 0xFF00) != (base.wrapping_add(cpu.y as u16) & 0xFF00) { 1 } else { 0 }
}

#[inline(always)]
fn page_cross_izy<M: Memory>(cpu: &Cpu, mem: &mut M) -> u8 {
    let ptr = read_u8(cpu, mem);
    let lo = mem.read(ptr as u16) as u16;
    let hi = mem.read(ptr.wrapping_add(1) as u16) as u16;
    let base = (hi << 8) | lo;
    if (base & 0xFF00) != (base.wrapping_add(cpu.y as u16) & 0xFF00) { 1 } else { 0 }
}

// ── ALU operations ──────────────────────────────────────────────────

#[inline(always)]
fn do_adc(cpu: &mut Cpu, val: u8) {
    let carry = cpu.status.get(CARRY) as u8;
    if cpu.status.get(DECIMAL) {
        let mut lo = (cpu.a & 0x0F) + (val & 0x0F) + carry;
        if lo > 9 { lo += 6; }
        let mut hi = (cpu.a >> 4) + (val >> 4) + if lo > 0x0F { 1 } else { 0 };
        let bin_sum = (cpu.a as u16) + (val as u16) + (carry as u16);
        cpu.status.set(ZERO, (bin_sum as u8) == 0);
        cpu.status.set(NEGATIVE, (hi & 0x08) != 0);
        cpu.status.set(OVERFLOW,
            (!(cpu.a ^ val) & (cpu.a ^ ((hi << 4) | (lo & 0x0F))) & 0x80) != 0);
        if hi > 9 { hi += 6; }
        cpu.status.set(CARRY, hi > 0x0F);
        cpu.a = ((hi & 0x0F) << 4) | (lo & 0x0F);
    } else {
        let (sum1, c1) = cpu.a.overflowing_add(val);
        let (sum2, c2) = sum1.overflowing_add(carry);
        cpu.status.set(CARRY, c1 || c2);
        cpu.status.set(OVERFLOW, (!(cpu.a ^ val) & (cpu.a ^ sum2) & 0x80) != 0);
        cpu.a = sum2;
        cpu.status.set_zn(cpu.a);
    }
}

#[inline(always)]
fn do_sbc(cpu: &mut Cpu, val: u8) {
    let borrow = !cpu.status.get(CARRY) as u8;
    if cpu.status.get(DECIMAL) {
        let mut lo = (cpu.a & 0x0F).wrapping_sub(val & 0x0F).wrapping_sub(borrow);
        let lo_borrow = if (lo as i8) < 0 { lo = lo.wrapping_sub(6); 1u8 } else { 0 };
        let mut hi = (cpu.a >> 4).wrapping_sub(val >> 4).wrapping_sub(lo_borrow);
        if (hi as i8) < 0 { hi = hi.wrapping_sub(6); }
        let bin_diff = (cpu.a as i16) - (val as i16) - (borrow as i16);
        cpu.status.set(CARRY, bin_diff >= 0);
        cpu.status.set(ZERO, (bin_diff as u8) == 0);
        cpu.status.set(NEGATIVE, (bin_diff as u8) & 0x80 != 0);
        cpu.status.set(OVERFLOW,
            ((cpu.a ^ val) & (cpu.a ^ (bin_diff as u8)) & 0x80) != 0);
        cpu.a = ((hi & 0x0F) << 4) | (lo & 0x0F);
    } else {
        let (diff1, b1) = cpu.a.overflowing_sub(val);
        let (diff2, b2) = diff1.overflowing_sub(borrow);
        cpu.status.set(CARRY, !(b1 || b2));
        cpu.status.set(OVERFLOW, ((cpu.a ^ val) & (cpu.a ^ diff2) & 0x80) != 0);
        cpu.a = diff2;
        cpu.status.set_zn(cpu.a);
    }
}

#[inline(always)]
fn do_cmp(cpu: &mut Cpu, reg: u8, val: u8) {
    let result = reg.wrapping_sub(val);
    cpu.status.set(CARRY, reg >= val);
    cpu.status.set_zn(result);
}

#[inline(always)]
fn do_asl_mem<M: Memory>(cpu: &mut Cpu, mem: &mut M, addr: u16) {
    let mut val = mem.read(addr);
    cpu.status.set(CARRY, val & 0x80 != 0);
    val <<= 1;
    mem.write(addr, val);
    cpu.status.set_zn(val);
}

#[inline(always)]
fn do_lsr_mem<M: Memory>(cpu: &mut Cpu, mem: &mut M, addr: u16) {
    let mut val = mem.read(addr);
    cpu.status.set(CARRY, val & 0x01 != 0);
    val >>= 1;
    mem.write(addr, val);
    cpu.status.set(ZERO, val == 0);
    cpu.status.set(NEGATIVE, false);
}

#[inline(always)]
fn do_rol_mem<M: Memory>(cpu: &mut Cpu, mem: &mut M, addr: u16) {
    let mut val = mem.read(addr);
    let old_carry = cpu.status.get(CARRY) as u8;
    cpu.status.set(CARRY, val & 0x80 != 0);
    val = (val << 1) | old_carry;
    mem.write(addr, val);
    cpu.status.set_zn(val);
}

#[inline(always)]
fn do_ror_mem<M: Memory>(cpu: &mut Cpu, mem: &mut M, addr: u16) {
    let mut val = mem.read(addr);
    let old_carry = cpu.status.get(CARRY) as u8;
    cpu.status.set(CARRY, val & 0x01 != 0);
    val = (val >> 1) | (old_carry << 7);
    mem.write(addr, val);
    cpu.status.set_zn(val);
}

#[inline(always)]
fn do_branch<M: Memory>(cpu: &mut Cpu, mem: &mut M, take: bool) -> u8 {
    if take {
        let off = read_u8(cpu, mem);
        let next_pc = cpu.pc.wrapping_add(2);
        let target = next_pc.wrapping_add(off as i8 as u16);
        let page_cross = (next_pc & 0xFF00) != (target & 0xFF00);
        cpu.pc = target;
        3 + page_cross as u8
    } else {
        cpu.pc = cpu.pc.wrapping_add(2);
        2
    }
}

// ── Macros to generate handlers for each addressing mode ────────────

macro_rules! ld_handler {
    ($name:ident, $reg:ident, $addr_fn:ident, $size:expr, $cycles:expr) => {
        fn $name<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 {
            let addr = $addr_fn(cpu, mem);
            cpu.$reg = mem.read(addr);
            cpu.status.set_zn(cpu.$reg);
            cpu.pc = cpu.pc.wrapping_add($size);
            $cycles
        }
    };
    // Immediate variant
    (imm $name:ident, $reg:ident) => {
        fn $name<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 {
            cpu.$reg = read_u8(cpu, mem);
            cpu.status.set_zn(cpu.$reg);
            cpu.pc = cpu.pc.wrapping_add(2);
            2
        }
    };
    // With page cross penalty
    (pc $name:ident, $reg:ident, $addr_fn:ident, $pc_fn:ident, $size:expr, $cycles:expr) => {
        fn $name<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 {
            let addr = $addr_fn(cpu, mem);
            cpu.$reg = mem.read(addr);
            cpu.status.set_zn(cpu.$reg);
            let penalty = $pc_fn(cpu, mem);
            cpu.pc = cpu.pc.wrapping_add($size);
            $cycles + penalty
        }
    };
}

macro_rules! st_handler {
    ($name:ident, $reg:ident, $addr_fn:ident, $size:expr, $cycles:expr) => {
        fn $name<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 {
            let addr = $addr_fn(cpu, mem);
            mem.write(addr, cpu.$reg);
            cpu.pc = cpu.pc.wrapping_add($size);
            $cycles
        }
    };
}

macro_rules! alu_handler {
    ($name:ident, $op:ident, $addr_fn:ident, $size:expr, $cycles:expr) => {
        fn $name<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 {
            let addr = $addr_fn(cpu, mem);
            let val = mem.read(addr);
            $op(cpu, val);
            cpu.pc = cpu.pc.wrapping_add($size);
            $cycles
        }
    };
    (imm $name:ident, $op:ident) => {
        fn $name<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 {
            let val = read_u8(cpu, mem);
            $op(cpu, val);
            cpu.pc = cpu.pc.wrapping_add(2);
            2
        }
    };
    (pc $name:ident, $op:ident, $addr_fn:ident, $pc_fn:ident, $size:expr, $cycles:expr) => {
        fn $name<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 {
            let addr = $addr_fn(cpu, mem);
            let val = mem.read(addr);
            $op(cpu, val);
            let penalty = $pc_fn(cpu, mem);
            cpu.pc = cpu.pc.wrapping_add($size);
            $cycles + penalty
        }
    };
}

macro_rules! logic_handler {
    ($name:ident, $op:tt, $addr_fn:ident, $size:expr, $cycles:expr) => {
        fn $name<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 {
            let addr = $addr_fn(cpu, mem);
            cpu.a $op mem.read(addr);
            cpu.status.set_zn(cpu.a);
            cpu.pc = cpu.pc.wrapping_add($size);
            $cycles
        }
    };
    (imm $name:ident, $op:tt) => {
        fn $name<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 {
            cpu.a $op read_u8(cpu, mem);
            cpu.status.set_zn(cpu.a);
            cpu.pc = cpu.pc.wrapping_add(2);
            2
        }
    };
    (pc $name:ident, $op:tt, $addr_fn:ident, $pc_fn:ident, $size:expr, $cycles:expr) => {
        fn $name<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 {
            let addr = $addr_fn(cpu, mem);
            cpu.a $op mem.read(addr);
            cpu.status.set_zn(cpu.a);
            let penalty = $pc_fn(cpu, mem);
            cpu.pc = cpu.pc.wrapping_add($size);
            $cycles + penalty
        }
    };
}

// ── Individual handlers ─────────────────────────────────────────────

// ADC
alu_handler!(imm adc_imm, do_adc);
alu_handler!(adc_zp, do_adc, addr_zp, 2, 3);
alu_handler!(adc_zpx, do_adc, addr_zpx, 2, 4);
alu_handler!(adc_abs, do_adc, addr_abs, 3, 4);
alu_handler!(pc adc_abx, do_adc, addr_abx, page_cross_abx, 3, 4);
alu_handler!(pc adc_aby, do_adc, addr_aby, page_cross_aby, 3, 4);
alu_handler!(adc_izx, do_adc, addr_izx, 2, 6);
alu_handler!(pc adc_izy, do_adc, addr_izy, page_cross_izy, 2, 5);

// SBC
alu_handler!(imm sbc_imm, do_sbc);
alu_handler!(sbc_zp, do_sbc, addr_zp, 2, 3);
alu_handler!(sbc_zpx, do_sbc, addr_zpx, 2, 4);
alu_handler!(sbc_abs, do_sbc, addr_abs, 3, 4);
alu_handler!(pc sbc_abx, do_sbc, addr_abx, page_cross_abx, 3, 4);
alu_handler!(pc sbc_aby, do_sbc, addr_aby, page_cross_aby, 3, 4);
alu_handler!(sbc_izx, do_sbc, addr_izx, 2, 6);
alu_handler!(pc sbc_izy, do_sbc, addr_izy, page_cross_izy, 2, 5);

// AND
logic_handler!(imm and_imm, &=);
logic_handler!(and_zp, &=, addr_zp, 2, 3);
logic_handler!(and_zpx, &=, addr_zpx, 2, 4);
logic_handler!(and_abs, &=, addr_abs, 3, 4);
logic_handler!(pc and_abx, &=, addr_abx, page_cross_abx, 3, 4);
logic_handler!(pc and_aby, &=, addr_aby, page_cross_aby, 3, 4);
logic_handler!(and_izx, &=, addr_izx, 2, 6);
logic_handler!(pc and_izy, &=, addr_izy, page_cross_izy, 2, 5);

// ORA
logic_handler!(imm ora_imm, |=);
logic_handler!(ora_zp, |=, addr_zp, 2, 3);
logic_handler!(ora_zpx, |=, addr_zpx, 2, 4);
logic_handler!(ora_abs, |=, addr_abs, 3, 4);
logic_handler!(pc ora_abx, |=, addr_abx, page_cross_abx, 3, 4);
logic_handler!(pc ora_aby, |=, addr_aby, page_cross_aby, 3, 4);
logic_handler!(ora_izx, |=, addr_izx, 2, 6);
logic_handler!(pc ora_izy, |=, addr_izy, page_cross_izy, 2, 5);

// EOR
logic_handler!(imm eor_imm, ^=);
logic_handler!(eor_zp, ^=, addr_zp, 2, 3);
logic_handler!(eor_zpx, ^=, addr_zpx, 2, 4);
logic_handler!(eor_abs, ^=, addr_abs, 3, 4);
logic_handler!(pc eor_abx, ^=, addr_abx, page_cross_abx, 3, 4);
logic_handler!(pc eor_aby, ^=, addr_aby, page_cross_aby, 3, 4);
logic_handler!(eor_izx, ^=, addr_izx, 2, 6);
logic_handler!(pc eor_izy, ^=, addr_izy, page_cross_izy, 2, 5);

// CMP
fn cmp_imm<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let v = read_u8(cpu, mem); do_cmp(cpu, cpu.a, v); cpu.pc += 2; 2 }
fn cmp_zp<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let a = addr_zp(cpu, mem); let v = mem.read(a); do_cmp(cpu, cpu.a, v); cpu.pc += 2; 3 }
fn cmp_zpx<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let a = addr_zpx(cpu, mem); let v = mem.read(a); do_cmp(cpu, cpu.a, v); cpu.pc += 2; 4 }
fn cmp_abs<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let a = addr_abs(cpu, mem); let v = mem.read(a); do_cmp(cpu, cpu.a, v); cpu.pc += 3; 4 }
fn cmp_abx<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let p = page_cross_abx(cpu, mem); let a = addr_abx(cpu, mem); let v = mem.read(a); do_cmp(cpu, cpu.a, v); cpu.pc += 3; 4 + p }
fn cmp_aby<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let p = page_cross_aby(cpu, mem); let a = addr_aby(cpu, mem); let v = mem.read(a); do_cmp(cpu, cpu.a, v); cpu.pc += 3; 4 + p }
fn cmp_izx<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let a = addr_izx(cpu, mem); let v = mem.read(a); do_cmp(cpu, cpu.a, v); cpu.pc += 2; 6 }
fn cmp_izy<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let p = page_cross_izy(cpu, mem); let a = addr_izy(cpu, mem); let v = mem.read(a); do_cmp(cpu, cpu.a, v); cpu.pc += 2; 5 + p }

// CPX
fn cpx_imm<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let v = read_u8(cpu, mem); do_cmp(cpu, cpu.x, v); cpu.pc += 2; 2 }
fn cpx_zp<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let a = addr_zp(cpu, mem); let v = mem.read(a); do_cmp(cpu, cpu.x, v); cpu.pc += 2; 3 }
fn cpx_abs<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let a = addr_abs(cpu, mem); let v = mem.read(a); do_cmp(cpu, cpu.x, v); cpu.pc += 3; 4 }

// CPY
fn cpy_imm<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let v = read_u8(cpu, mem); do_cmp(cpu, cpu.y, v); cpu.pc += 2; 2 }
fn cpy_zp<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let a = addr_zp(cpu, mem); let v = mem.read(a); do_cmp(cpu, cpu.y, v); cpu.pc += 2; 3 }
fn cpy_abs<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let a = addr_abs(cpu, mem); let v = mem.read(a); do_cmp(cpu, cpu.y, v); cpu.pc += 3; 4 }

// LDA
ld_handler!(imm lda_imm, a);
ld_handler!(lda_zp, a, addr_zp, 2, 3);
ld_handler!(lda_zpx, a, addr_zpx, 2, 4);
ld_handler!(lda_abs, a, addr_abs, 3, 4);
ld_handler!(pc lda_abx, a, addr_abx, page_cross_abx, 3, 4);
ld_handler!(pc lda_aby, a, addr_aby, page_cross_aby, 3, 4);
ld_handler!(lda_izx, a, addr_izx, 2, 6);
ld_handler!(pc lda_izy, a, addr_izy, page_cross_izy, 2, 5);

// LDX
ld_handler!(imm ldx_imm, x);
ld_handler!(ldx_zp, x, addr_zp, 2, 3);
ld_handler!(ldx_zpy, x, addr_zpy, 2, 4);
ld_handler!(ldx_abs, x, addr_abs, 3, 4);
ld_handler!(pc ldx_aby, x, addr_aby, page_cross_aby, 3, 4);

// LDY
ld_handler!(imm ldy_imm, y);
ld_handler!(ldy_zp, y, addr_zp, 2, 3);
ld_handler!(ldy_zpx, y, addr_zpx, 2, 4);
ld_handler!(ldy_abs, y, addr_abs, 3, 4);
ld_handler!(pc ldy_abx, y, addr_abx, page_cross_abx, 3, 4);

// STA
st_handler!(sta_zp, a, addr_zp, 2, 3);
st_handler!(sta_zpx, a, addr_zpx, 2, 4);
st_handler!(sta_abs, a, addr_abs, 3, 4);
st_handler!(sta_abx, a, addr_abx, 3, 5);
st_handler!(sta_aby, a, addr_aby, 3, 5);
st_handler!(sta_izx, a, addr_izx, 2, 6);
st_handler!(sta_izy, a, addr_izy, 2, 6);

// STX
st_handler!(stx_zp, x, addr_zp, 2, 3);
st_handler!(stx_zpy, x, addr_zpy, 2, 4);
st_handler!(stx_abs, x, addr_abs, 3, 4);

// STY
st_handler!(sty_zp, y, addr_zp, 2, 3);
st_handler!(sty_zpx, y, addr_zpx, 2, 4);
st_handler!(sty_abs, y, addr_abs, 3, 4);

// INC/DEC memory
fn inc_zp<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_zp(cpu, mem); let v = mem.read(ad).wrapping_add(1); mem.write(ad, v); cpu.status.set_zn(v); cpu.pc += 2; 5 }
fn inc_zpx<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_zpx(cpu, mem); let v = mem.read(ad).wrapping_add(1); mem.write(ad, v); cpu.status.set_zn(v); cpu.pc += 2; 6 }
fn inc_abs<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_abs(cpu, mem); let v = mem.read(ad).wrapping_add(1); mem.write(ad, v); cpu.status.set_zn(v); cpu.pc += 3; 6 }
fn inc_abx<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_abx(cpu, mem); let v = mem.read(ad).wrapping_add(1); mem.write(ad, v); cpu.status.set_zn(v); cpu.pc += 3; 7 }

fn dec_zp<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_zp(cpu, mem); let v = mem.read(ad).wrapping_sub(1); mem.write(ad, v); cpu.status.set_zn(v); cpu.pc += 2; 5 }
fn dec_zpx<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_zpx(cpu, mem); let v = mem.read(ad).wrapping_sub(1); mem.write(ad, v); cpu.status.set_zn(v); cpu.pc += 2; 6 }
fn dec_abs<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_abs(cpu, mem); let v = mem.read(ad).wrapping_sub(1); mem.write(ad, v); cpu.status.set_zn(v); cpu.pc += 3; 6 }
fn dec_abx<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_abx(cpu, mem); let v = mem.read(ad).wrapping_sub(1); mem.write(ad, v); cpu.status.set_zn(v); cpu.pc += 3; 7 }

// INX/INY/DEX/DEY
fn inx<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.x = cpu.x.wrapping_add(1); cpu.status.set_zn(cpu.x); cpu.pc += 1; 2 }
fn iny<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.y = cpu.y.wrapping_add(1); cpu.status.set_zn(cpu.y); cpu.pc += 1; 2 }
fn dex<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.x = cpu.x.wrapping_sub(1); cpu.status.set_zn(cpu.x); cpu.pc += 1; 2 }
fn dey<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.y = cpu.y.wrapping_sub(1); cpu.status.set_zn(cpu.y); cpu.pc += 1; 2 }

// ASL
fn asl_acc<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.status.set(CARRY, cpu.a & 0x80 != 0); cpu.a <<= 1; cpu.status.set_zn(cpu.a); cpu.pc += 1; 2 }
fn asl_zp<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_zp(cpu, mem); do_asl_mem(cpu, mem, ad); cpu.pc += 2; 5 }
fn asl_zpx<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_zpx(cpu, mem); do_asl_mem(cpu, mem, ad); cpu.pc += 2; 6 }
fn asl_abs<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_abs(cpu, mem); do_asl_mem(cpu, mem, ad); cpu.pc += 3; 6 }
fn asl_abx<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_abx(cpu, mem); do_asl_mem(cpu, mem, ad); cpu.pc += 3; 7 }

// LSR
fn lsr_acc<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.status.set(CARRY, cpu.a & 0x01 != 0); cpu.a >>= 1; cpu.status.set(ZERO, cpu.a == 0); cpu.status.set(NEGATIVE, false); cpu.pc += 1; 2 }
fn lsr_zp<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_zp(cpu, mem); do_lsr_mem(cpu, mem, ad); cpu.pc += 2; 5 }
fn lsr_zpx<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_zpx(cpu, mem); do_lsr_mem(cpu, mem, ad); cpu.pc += 2; 6 }
fn lsr_abs<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_abs(cpu, mem); do_lsr_mem(cpu, mem, ad); cpu.pc += 3; 6 }
fn lsr_abx<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_abx(cpu, mem); do_lsr_mem(cpu, mem, ad); cpu.pc += 3; 7 }

// ROL
fn rol_acc<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { let c = cpu.status.get(CARRY) as u8; cpu.status.set(CARRY, cpu.a & 0x80 != 0); cpu.a = (cpu.a << 1) | c; cpu.status.set_zn(cpu.a); cpu.pc += 1; 2 }
fn rol_zp<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_zp(cpu, mem); do_rol_mem(cpu, mem, ad); cpu.pc += 2; 5 }
fn rol_zpx<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_zpx(cpu, mem); do_rol_mem(cpu, mem, ad); cpu.pc += 2; 6 }
fn rol_abs<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_abs(cpu, mem); do_rol_mem(cpu, mem, ad); cpu.pc += 3; 6 }
fn rol_abx<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_abx(cpu, mem); do_rol_mem(cpu, mem, ad); cpu.pc += 3; 7 }

// ROR
fn ror_acc<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { let c = cpu.status.get(CARRY) as u8; cpu.status.set(CARRY, cpu.a & 0x01 != 0); cpu.a = (cpu.a >> 1) | (c << 7); cpu.status.set_zn(cpu.a); cpu.pc += 1; 2 }
fn ror_zp<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_zp(cpu, mem); do_ror_mem(cpu, mem, ad); cpu.pc += 2; 5 }
fn ror_zpx<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_zpx(cpu, mem); do_ror_mem(cpu, mem, ad); cpu.pc += 2; 6 }
fn ror_abs<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_abs(cpu, mem); do_ror_mem(cpu, mem, ad); cpu.pc += 3; 6 }
fn ror_abx<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_abx(cpu, mem); do_ror_mem(cpu, mem, ad); cpu.pc += 3; 7 }

// BIT
fn bit_zp<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_zp(cpu, mem); let v = mem.read(ad); cpu.status.set(ZERO, (cpu.a & v) == 0); cpu.status.set(OVERFLOW, v & 0x40 != 0); cpu.status.set(NEGATIVE, v & 0x80 != 0); cpu.pc += 2; 3 }
fn bit_abs<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { let ad = addr_abs(cpu, mem); let v = mem.read(ad); cpu.status.set(ZERO, (cpu.a & v) == 0); cpu.status.set(OVERFLOW, v & 0x40 != 0); cpu.status.set(NEGATIVE, v & 0x80 != 0); cpu.pc += 3; 4 }

// Transfers
fn tax<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.x = cpu.a; cpu.status.set_zn(cpu.x); cpu.pc += 1; 2 }
fn txa<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.a = cpu.x; cpu.status.set_zn(cpu.a); cpu.pc += 1; 2 }
fn tay<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.y = cpu.a; cpu.status.set_zn(cpu.y); cpu.pc += 1; 2 }
fn tya<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.a = cpu.y; cpu.status.set_zn(cpu.a); cpu.pc += 1; 2 }
fn txs<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.sp = cpu.x; cpu.pc += 1; 2 }
fn tsx<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.x = cpu.sp; cpu.status.set_zn(cpu.x); cpu.pc += 1; 2 }

// Stack
fn pha<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { mem.write(0x0100 | cpu.sp as u16, cpu.a); cpu.sp = cpu.sp.wrapping_sub(1); cpu.pc += 1; 3 }
fn pla<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { cpu.sp = cpu.sp.wrapping_add(1); cpu.a = mem.read(0x0100 | cpu.sp as u16); cpu.status.set_zn(cpu.a); cpu.pc += 1; 4 }
fn php<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { mem.write(0x0100 | cpu.sp as u16, cpu.status.to_byte_with_break()); cpu.sp = cpu.sp.wrapping_sub(1); cpu.pc += 1; 3 }
fn plp<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { cpu.sp = cpu.sp.wrapping_add(1); cpu.status = Status::from_byte(mem.read(0x0100 | cpu.sp as u16)); cpu.pc += 1; 4 }

// Branches
fn bpl<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { do_branch(cpu, mem, !cpu.status.get(NEGATIVE)) }
fn bmi<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { do_branch(cpu, mem, cpu.status.get(NEGATIVE)) }
fn bvc<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { do_branch(cpu, mem, !cpu.status.get(OVERFLOW)) }
fn bvs<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { do_branch(cpu, mem, cpu.status.get(OVERFLOW)) }
fn bcc<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { do_branch(cpu, mem, !cpu.status.get(CARRY)) }
fn bcs<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { do_branch(cpu, mem, cpu.status.get(CARRY)) }
fn bne<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { do_branch(cpu, mem, !cpu.status.get(ZERO)) }
fn beq<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { do_branch(cpu, mem, cpu.status.get(ZERO)) }

// Flags
fn clc<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.status.set(CARRY, false); cpu.pc += 1; 2 }
fn sec<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.status.set(CARRY, true); cpu.pc += 1; 2 }
fn cli<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.status.set(INTERRUPT, false); cpu.pc += 1; 2 }
fn sei<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.status.set(INTERRUPT, true); cpu.pc += 1; 2 }
fn clv<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.status.set(OVERFLOW, false); cpu.pc += 1; 2 }
fn cld<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.status.set(DECIMAL, false); cpu.pc += 1; 2 }
fn sed<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.status.set(DECIMAL, true); cpu.pc += 1; 2 }

// JMP
fn jmp_abs<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 { cpu.pc = addr_abs(cpu, mem); 3 }
fn jmp_ind<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 {
    let addr = read_u16(cpu, mem);
    let lo = mem.read(addr) as u16;
    let hi_addr = (addr & 0xFF00) | ((addr.wrapping_add(1)) & 0x00FF);
    let hi = mem.read(hi_addr) as u16;
    cpu.pc = (hi << 8) | lo;
    5
}

// JSR/RTS/RTI/BRK
fn jsr<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 {
    let ret = cpu.pc + 2;
    mem.write(0x0100 | cpu.sp as u16, (ret >> 8) as u8);
    cpu.sp = cpu.sp.wrapping_sub(1);
    mem.write(0x0100 | cpu.sp as u16, ret as u8);
    cpu.sp = cpu.sp.wrapping_sub(1);
    cpu.pc = addr_abs(cpu, mem);
    6
}

fn rts<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 {
    cpu.sp = cpu.sp.wrapping_add(1);
    let lo = mem.read(0x0100 | cpu.sp as u16) as u16;
    cpu.sp = cpu.sp.wrapping_add(1);
    let hi = mem.read(0x0100 | cpu.sp as u16) as u16;
    cpu.pc = ((hi << 8) | lo).wrapping_add(1);
    6
}

fn rti<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 {
    cpu.sp = cpu.sp.wrapping_add(1);
    cpu.status = Status::from_byte(mem.read(0x0100 | cpu.sp as u16));
    cpu.sp = cpu.sp.wrapping_add(1);
    let lo = mem.read(0x0100 | cpu.sp as u16) as u16;
    cpu.sp = cpu.sp.wrapping_add(1);
    let hi = mem.read(0x0100 | cpu.sp as u16) as u16;
    cpu.pc = (hi << 8) | lo;
    6
}

fn brk<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 {
    let ret = cpu.pc + 2;
    mem.write(0x0100 | cpu.sp as u16, (ret >> 8) as u8);
    cpu.sp = cpu.sp.wrapping_sub(1);
    mem.write(0x0100 | cpu.sp as u16, ret as u8);
    cpu.sp = cpu.sp.wrapping_sub(1);
    mem.write(0x0100 | cpu.sp as u16, cpu.status.to_byte_with_break());
    cpu.sp = cpu.sp.wrapping_sub(1);
    cpu.status.set(INTERRUPT, true);
    let lo = mem.read(0xFFFE) as u16;
    let hi = mem.read(0xFFFF) as u16;
    cpu.pc = (hi << 8) | lo;
    7
}

fn nop<M: Memory>(cpu: &mut Cpu, _: &mut M) -> u8 { cpu.pc += 1; 2 }

fn illegal<M: Memory>(cpu: &mut Cpu, mem: &mut M) -> u8 {
    panic!("illegal opcode: 0x{:02X} at PC=0x{:04X}", mem.read(cpu.pc), cpu.pc);
}

// ── Dispatch table ──────────────────────────────────────────────────

pub fn dispatch_table<M: Memory>() -> [Handler<M>; 256] {
    let mut t: [Handler<M>; 256] = [illegal; 256];

    t[0x00] = brk;
    t[0x01] = ora_izx;   t[0x05] = ora_zp;    t[0x09] = ora_imm;
    t[0x0D] = ora_abs;   t[0x11] = ora_izy;   t[0x15] = ora_zpx;
    t[0x19] = ora_aby;   t[0x1D] = ora_abx;

    t[0x06] = asl_zp;    t[0x0A] = asl_acc;   t[0x0E] = asl_abs;
    t[0x16] = asl_zpx;   t[0x1E] = asl_abx;

    t[0x08] = php;       t[0x28] = plp;

    t[0x10] = bpl;       t[0x30] = bmi;
    t[0x18] = clc;       t[0x38] = sec;

    t[0x20] = jsr;

    t[0x21] = and_izx;   t[0x25] = and_zp;    t[0x29] = and_imm;
    t[0x2D] = and_abs;   t[0x31] = and_izy;   t[0x35] = and_zpx;
    t[0x39] = and_aby;   t[0x3D] = and_abx;

    t[0x24] = bit_zp;    t[0x2C] = bit_abs;

    t[0x26] = rol_zp;    t[0x2A] = rol_acc;   t[0x2E] = rol_abs;
    t[0x36] = rol_zpx;   t[0x3E] = rol_abx;

    t[0x40] = rti;

    t[0x41] = eor_izx;   t[0x45] = eor_zp;    t[0x49] = eor_imm;
    t[0x4D] = eor_abs;   t[0x51] = eor_izy;   t[0x55] = eor_zpx;
    t[0x59] = eor_aby;   t[0x5D] = eor_abx;

    t[0x46] = lsr_zp;    t[0x4A] = lsr_acc;   t[0x4E] = lsr_abs;
    t[0x56] = lsr_zpx;   t[0x5E] = lsr_abx;

    t[0x48] = pha;       t[0x68] = pla;

    t[0x4C] = jmp_abs;   t[0x6C] = jmp_ind;
    t[0x50] = bvc;       t[0x70] = bvs;
    t[0x58] = cli;       t[0x78] = sei;

    t[0x60] = rts;

    t[0x61] = adc_izx;   t[0x65] = adc_zp;    t[0x69] = adc_imm;
    t[0x6D] = adc_abs;   t[0x71] = adc_izy;   t[0x75] = adc_zpx;
    t[0x79] = adc_aby;   t[0x7D] = adc_abx;

    t[0x66] = ror_zp;    t[0x6A] = ror_acc;   t[0x6E] = ror_abs;
    t[0x76] = ror_zpx;   t[0x7E] = ror_abx;

    t[0x81] = sta_izx;   t[0x85] = sta_zp;    t[0x8D] = sta_abs;
    t[0x91] = sta_izy;   t[0x95] = sta_zpx;   t[0x99] = sta_aby;
    t[0x9D] = sta_abx;

    t[0x86] = stx_zp;    t[0x96] = stx_zpy;   t[0x8E] = stx_abs;
    t[0x84] = sty_zp;    t[0x94] = sty_zpx;   t[0x8C] = sty_abs;

    t[0x88] = dey;       t[0xA8] = tay;       t[0x8A] = txa;
    t[0xAA] = tax;       t[0x98] = tya;       t[0x9A] = txs;
    t[0xBA] = tsx;

    t[0x90] = bcc;       t[0xB0] = bcs;

    t[0xA1] = lda_izx;   t[0xA5] = lda_zp;    t[0xA9] = lda_imm;
    t[0xAD] = lda_abs;   t[0xB1] = lda_izy;   t[0xB5] = lda_zpx;
    t[0xB9] = lda_aby;   t[0xBD] = lda_abx;

    t[0xA2] = ldx_imm;   t[0xA6] = ldx_zp;    t[0xAE] = ldx_abs;
    t[0xB6] = ldx_zpy;   t[0xBE] = ldx_aby;

    t[0xA0] = ldy_imm;   t[0xA4] = ldy_zp;    t[0xAC] = ldy_abs;
    t[0xB4] = ldy_zpx;   t[0xBC] = ldy_abx;

    t[0xB8] = clv;

    t[0xC0] = cpy_imm;   t[0xC4] = cpy_zp;    t[0xCC] = cpy_abs;

    t[0xC1] = cmp_izx;   t[0xC5] = cmp_zp;    t[0xC9] = cmp_imm;
    t[0xCD] = cmp_abs;   t[0xD1] = cmp_izy;   t[0xD5] = cmp_zpx;
    t[0xD9] = cmp_aby;   t[0xDD] = cmp_abx;

    t[0xC6] = dec_zp;    t[0xCE] = dec_abs;   t[0xD6] = dec_zpx;
    t[0xDE] = dec_abx;

    t[0xC8] = iny;       t[0xE8] = inx;       t[0xCA] = dex;

    t[0xD0] = bne;       t[0xF0] = beq;
    t[0xD8] = cld;       t[0xF8] = sed;

    t[0xE0] = cpx_imm;   t[0xE4] = cpx_zp;    t[0xEC] = cpx_abs;

    t[0xE1] = sbc_izx;   t[0xE5] = sbc_zp;    t[0xE9] = sbc_imm;
    t[0xED] = sbc_abs;   t[0xF1] = sbc_izy;   t[0xF5] = sbc_zpx;
    t[0xF9] = sbc_aby;   t[0xFD] = sbc_abx;

    t[0xE6] = inc_zp;    t[0xEE] = inc_abs;   t[0xF6] = inc_zpx;
    t[0xFE] = inc_abx;

    t[0xEA] = nop;

    t
}
