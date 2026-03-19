#pragma once
#include "state_layout.cuh"
#include "memory_bus.cuh"

// Status flag bits (NV-BDIZC)
#define FLAG_CARRY     (1 << 0)
#define FLAG_ZERO      (1 << 1)
#define FLAG_INTERRUPT (1 << 2)
#define FLAG_DECIMAL   (1 << 3)
#define FLAG_BREAK     (1 << 4)
#define FLAG_UNUSED    (1 << 5)
#define FLAG_OVERFLOW  (1 << 6)
#define FLAG_NEGATIVE  (1 << 7)

// Cycle count table embedded in device code.
__device__ static const uint8_t CYCLE_TABLE[256] = {
    7, 6, 0, 0, 0, 3, 5, 0, 3, 2, 2, 0, 0, 4, 6, 0,
    2, 5, 0, 0, 0, 4, 6, 0, 2, 4, 0, 0, 0, 4, 7, 0,
    6, 6, 0, 0, 3, 3, 5, 0, 4, 2, 2, 0, 4, 4, 6, 0,
    2, 5, 0, 0, 0, 4, 6, 0, 2, 4, 0, 0, 0, 4, 7, 0,
    6, 6, 0, 0, 0, 3, 5, 0, 3, 2, 2, 0, 3, 4, 6, 0,
    2, 5, 0, 0, 0, 4, 6, 0, 2, 4, 0, 0, 0, 4, 7, 0,
    6, 6, 0, 0, 0, 3, 5, 0, 4, 2, 2, 0, 5, 4, 6, 0,
    2, 5, 0, 0, 0, 4, 6, 0, 2, 4, 0, 0, 0, 4, 7, 0,
    0, 6, 0, 0, 3, 3, 3, 0, 2, 0, 2, 0, 4, 4, 4, 0,
    2, 6, 0, 0, 4, 4, 4, 0, 2, 5, 2, 0, 5, 5, 0, 0,
    2, 6, 2, 0, 3, 3, 3, 0, 2, 2, 2, 0, 4, 4, 4, 0,
    2, 5, 0, 0, 4, 4, 4, 0, 2, 4, 2, 0, 4, 4, 4, 0,
    2, 6, 0, 0, 3, 3, 5, 0, 2, 2, 2, 0, 4, 4, 6, 0,
    2, 5, 0, 0, 0, 4, 6, 0, 2, 4, 0, 0, 0, 4, 7, 0,
    2, 6, 0, 0, 3, 3, 5, 0, 2, 2, 2, 0, 4, 4, 6, 0,
    2, 5, 0, 0, 0, 4, 6, 0, 2, 4, 0, 0, 0, 4, 7, 0,
};

// Instruction size table.
__device__ static const uint8_t SIZE_TABLE[256] = {
    1, 2, 1, 1, 1, 2, 2, 1, 1, 2, 1, 1, 1, 3, 3, 1,
    2, 2, 1, 1, 1, 2, 2, 1, 1, 3, 1, 1, 1, 3, 3, 1,
    3, 2, 1, 1, 2, 2, 2, 1, 1, 2, 1, 1, 3, 3, 3, 1,
    2, 2, 1, 1, 1, 2, 2, 1, 1, 3, 1, 1, 1, 3, 3, 1,
    1, 2, 1, 1, 1, 2, 2, 1, 1, 2, 1, 1, 3, 3, 3, 1,
    2, 2, 1, 1, 1, 2, 2, 1, 1, 3, 1, 1, 1, 3, 3, 1,
    1, 2, 1, 1, 1, 2, 2, 1, 1, 2, 1, 1, 3, 3, 3, 1,
    2, 2, 1, 1, 1, 2, 2, 1, 1, 3, 1, 1, 1, 3, 3, 1,
    1, 2, 1, 1, 2, 2, 2, 1, 1, 1, 1, 1, 3, 3, 3, 1,
    2, 2, 1, 1, 2, 2, 2, 1, 1, 3, 1, 1, 3, 3, 1, 1,
    2, 2, 2, 1, 2, 2, 2, 1, 1, 2, 1, 1, 3, 3, 3, 1,
    2, 2, 1, 1, 2, 2, 2, 1, 1, 3, 1, 1, 3, 3, 3, 1,
    2, 2, 1, 1, 2, 2, 2, 1, 1, 2, 1, 1, 3, 3, 3, 1,
    2, 2, 1, 1, 1, 2, 2, 1, 1, 3, 1, 1, 1, 3, 3, 1,
    2, 2, 1, 1, 2, 2, 2, 1, 1, 2, 1, 1, 3, 3, 3, 1,
    2, 2, 1, 1, 1, 2, 2, 1, 1, 3, 1, 1, 1, 3, 3, 1,
};

// --- Helper functions ---

__device__ __forceinline__
void set_flag(uint8_t* status, uint8_t flag, bool on) {
    if (on) *status |= flag;
    else    *status &= ~flag;
}

__device__ __forceinline__
bool get_flag(uint8_t status, uint8_t flag) {
    return (status & flag) != 0;
}

__device__ __forceinline__
void set_zn(uint8_t* status, uint8_t val) {
    set_flag(status, FLAG_ZERO, val == 0);
    set_flag(status, FLAG_NEGATIVE, (val & 0x80) != 0);
}

// Shorthand: bus_read with ROM context
#define BR(s, addr) bus_read(s, addr, rom_ptr, rom_len)

// Read a 16-bit little-endian value from two consecutive addresses.
__device__ __forceinline__
uint16_t read16(AtariState* s, uint16_t addr, const uint8_t* rom_ptr, uint32_t rom_len) {
    uint8_t lo = BR(s, addr);
    uint8_t hi = BR(s, (uint16_t)(addr + 1));
    return ((uint16_t)hi << 8) | lo;
}
#define R16(s, addr) read16(s, addr, rom_ptr, rom_len)

// Stack operations.
__device__ __forceinline__
void push8(AtariState* s, uint8_t val) {
    bus_write(s, 0x0100 | (uint16_t)s->sp, val);
    s->sp -= 1;
}

__device__ __forceinline__
uint8_t pull8(AtariState* s, const uint8_t* rom_ptr, uint32_t rom_len) {
    s->sp += 1;
    return BR(s, 0x0100 | (uint16_t)s->sp);
}
#define PULL8(s) pull8(s, rom_ptr, rom_len)

__device__ __forceinline__
void push16(AtariState* s, uint16_t val) {
    push8(s, (uint8_t)(val >> 8));
    push8(s, (uint8_t)(val & 0xFF));
}

__device__ __forceinline__
uint16_t pull16(AtariState* s, const uint8_t* rom_ptr, uint32_t rom_len) {
    uint8_t lo = PULL8(s);
    uint8_t hi = PULL8(s);
    return ((uint16_t)hi << 8) | lo;
}
#define PULL16(s) pull16(s, rom_ptr, rom_len)

// --- Address resolution helpers ---

__device__ __forceinline__
uint16_t addr_zp(AtariState* s, const uint8_t* rom_ptr, uint32_t rom_len) {
    return (uint16_t)BR(s, s->pc + 1);
}

__device__ __forceinline__
uint16_t addr_zpx(AtariState* s, const uint8_t* rom_ptr, uint32_t rom_len) {
    return (uint16_t)((BR(s, s->pc + 1) + s->x) & 0xFF);
}

__device__ __forceinline__
uint16_t addr_zpy(AtariState* s, const uint8_t* rom_ptr, uint32_t rom_len) {
    return (uint16_t)((BR(s, s->pc + 1) + s->y) & 0xFF);
}

__device__ __forceinline__
uint16_t addr_abs(AtariState* s, const uint8_t* rom_ptr, uint32_t rom_len) {
    return R16(s, s->pc + 1);
}

__device__ __forceinline__
uint16_t addr_abx(AtariState* s, const uint8_t* rom_ptr, uint32_t rom_len) {
    return (uint16_t)(R16(s, s->pc + 1) + s->x);
}

__device__ __forceinline__
uint16_t addr_aby(AtariState* s, const uint8_t* rom_ptr, uint32_t rom_len) {
    return (uint16_t)(R16(s, s->pc + 1) + s->y);
}

__device__ __forceinline__
uint16_t addr_izx(AtariState* s, const uint8_t* rom_ptr, uint32_t rom_len) {
    uint8_t ptr = BR(s, s->pc + 1) + s->x;
    uint8_t lo = BR(s, (uint16_t)ptr);
    uint8_t hi = BR(s, (uint16_t)((ptr + 1) & 0xFF));
    return ((uint16_t)hi << 8) | lo;
}

__device__ __forceinline__
uint16_t addr_izy(AtariState* s, const uint8_t* rom_ptr, uint32_t rom_len) {
    uint8_t zp = BR(s, s->pc + 1);
    uint8_t lo = BR(s, (uint16_t)zp);
    uint8_t hi = BR(s, (uint16_t)((zp + 1) & 0xFF));
    return (uint16_t)(((uint16_t)hi << 8) | lo) + s->y;
}

// Macros for address modes that thread ROM context
#define ADDR_ZP(s)  addr_zp(s, rom_ptr, rom_len)
#define ADDR_ZPX(s) addr_zpx(s, rom_ptr, rom_len)
#define ADDR_ZPY(s) addr_zpy(s, rom_ptr, rom_len)
#define ADDR_ABS(s) addr_abs(s, rom_ptr, rom_len)
#define ADDR_ABX(s) addr_abx(s, rom_ptr, rom_len)
#define ADDR_ABY(s) addr_aby(s, rom_ptr, rom_len)
#define ADDR_IZX(s) addr_izx(s, rom_ptr, rom_len)
#define ADDR_IZY(s) addr_izy(s, rom_ptr, rom_len)

// Immediate value
#define IMM(s) BR(s, s->pc + 1)

// --- Page cross penalty helpers ---

__device__ __forceinline__
uint8_t page_cross_abx(AtariState* s, const uint8_t* rom_ptr, uint32_t rom_len) {
    uint16_t base = R16(s, s->pc + 1);
    return ((base & 0xFF00) != ((base + s->x) & 0xFF00)) ? 1 : 0;
}

__device__ __forceinline__
uint8_t page_cross_aby(AtariState* s, const uint8_t* rom_ptr, uint32_t rom_len) {
    uint16_t base = R16(s, s->pc + 1);
    return ((base & 0xFF00) != ((base + s->y) & 0xFF00)) ? 1 : 0;
}

__device__ __forceinline__
uint8_t page_cross_izy(AtariState* s, const uint8_t* rom_ptr, uint32_t rom_len) {
    uint8_t zp = BR(s, s->pc + 1);
    uint8_t lo = BR(s, (uint16_t)zp);
    uint8_t hi = BR(s, (uint16_t)((zp + 1) & 0xFF));
    uint16_t base = ((uint16_t)hi << 8) | lo;
    return ((base & 0xFF00) != ((base + s->y) & 0xFF00)) ? 1 : 0;
}

#define PX_ABX(s) page_cross_abx(s, rom_ptr, rom_len)
#define PX_ABY(s) page_cross_aby(s, rom_ptr, rom_len)
#define PX_IZY(s) page_cross_izy(s, rom_ptr, rom_len)

// --- ADC / SBC helpers ---

__device__ __forceinline__
void do_adc(AtariState* s, uint8_t val) {
    uint8_t carry = get_flag(s->status, FLAG_CARRY) ? 1 : 0;

    if (get_flag(s->status, FLAG_DECIMAL)) {
        uint8_t lo = (s->a & 0x0F) + (val & 0x0F) + carry;
        if (lo > 9) lo += 6;
        uint8_t hi = (s->a >> 4) + (val >> 4) + (lo > 0x0F ? 1 : 0);

        uint16_t bin_sum = (uint16_t)s->a + (uint16_t)val + (uint16_t)carry;
        set_flag(&s->status, FLAG_ZERO, (uint8_t)bin_sum == 0);
        set_flag(&s->status, FLAG_NEGATIVE, (hi & 0x08) != 0);
        set_flag(&s->status, FLAG_OVERFLOW,
            ((~(s->a ^ val) & (s->a ^ ((hi << 4) | (lo & 0x0F)))) & 0x80) != 0);

        if (hi > 9) hi += 6;
        set_flag(&s->status, FLAG_CARRY, hi > 0x0F);
        s->a = ((hi & 0x0F) << 4) | (lo & 0x0F);
    } else {
        uint16_t sum = (uint16_t)s->a + (uint16_t)val + (uint16_t)carry;
        set_flag(&s->status, FLAG_CARRY, sum > 0xFF);
        set_flag(&s->status, FLAG_OVERFLOW,
            ((~(s->a ^ val) & (s->a ^ (uint8_t)sum)) & 0x80) != 0);
        s->a = (uint8_t)sum;
        set_zn(&s->status, s->a);
    }
}

__device__ __forceinline__
void do_sbc(AtariState* s, uint8_t val) {
    uint8_t borrow = get_flag(s->status, FLAG_CARRY) ? 0 : 1;

    if (get_flag(s->status, FLAG_DECIMAL)) {
        uint8_t lo = (s->a & 0x0F) - (val & 0x0F) - borrow;
        uint8_t lo_borrow = ((int8_t)lo < 0) ? 1 : 0;
        if (lo_borrow) lo -= 6;
        uint8_t hi = (s->a >> 4) - (val >> 4) - lo_borrow;
        if ((int8_t)hi < 0) hi -= 6;

        int16_t bin_diff = (int16_t)s->a - (int16_t)val - (int16_t)borrow;
        set_flag(&s->status, FLAG_CARRY, bin_diff >= 0);
        set_flag(&s->status, FLAG_ZERO, (uint8_t)bin_diff == 0);
        set_flag(&s->status, FLAG_NEGATIVE, ((uint8_t)bin_diff & 0x80) != 0);
        set_flag(&s->status, FLAG_OVERFLOW,
            (((s->a ^ val) & (s->a ^ (uint8_t)bin_diff)) & 0x80) != 0);
        s->a = ((hi & 0x0F) << 4) | (lo & 0x0F);
    } else {
        uint16_t diff = (uint16_t)s->a - (uint16_t)val - (uint16_t)borrow;
        set_flag(&s->status, FLAG_CARRY, diff < 0x100);
        set_flag(&s->status, FLAG_OVERFLOW,
            (((s->a ^ val) & (s->a ^ (uint8_t)diff)) & 0x80) != 0);
        s->a = (uint8_t)diff;
        set_zn(&s->status, s->a);
    }
}

// --- Compare helper ---

__device__ __forceinline__
void do_cmp(uint8_t* status, uint8_t reg, uint8_t val) {
    uint8_t result = reg - val;
    set_flag(status, FLAG_CARRY, reg >= val);
    set_zn(status, result);
}

// --- Branch helper ---
__device__ __forceinline__
uint8_t do_branch(AtariState* s, bool taken, uint8_t base_cycles, const uint8_t* rom_ptr, uint32_t rom_len) {
    if (!taken) {
        s->pc += 2;
        return base_cycles;
    }
    uint8_t off = BR(s, s->pc + 1);
    uint16_t next_pc = s->pc + 2;
    uint16_t target = next_pc + (uint16_t)(int16_t)(int8_t)off;
    uint8_t page_cross = ((next_pc & 0xFF00) != (target & 0xFF00)) ? 1 : 0;
    s->pc = target;
    return base_cycles + 1 + page_cross;
}
#define DO_BRANCH(s, taken, bc) do_branch(s, taken, bc, rom_ptr, rom_len)

// --- Read-modify-write helpers ---

__device__ __forceinline__
void do_asl_mem(AtariState* s, uint16_t addr, const uint8_t* rom_ptr, uint32_t rom_len) {
    uint8_t val = BR(s, addr);
    set_flag(&s->status, FLAG_CARRY, (val & 0x80) != 0);
    val <<= 1;
    bus_write(s, addr, val);
    set_zn(&s->status, val);
}

__device__ __forceinline__
void do_lsr_mem(AtariState* s, uint16_t addr, const uint8_t* rom_ptr, uint32_t rom_len) {
    uint8_t val = BR(s, addr);
    set_flag(&s->status, FLAG_CARRY, (val & 0x01) != 0);
    val >>= 1;
    bus_write(s, addr, val);
    set_flag(&s->status, FLAG_ZERO, val == 0);
    set_flag(&s->status, FLAG_NEGATIVE, false);
}

__device__ __forceinline__
void do_rol_mem(AtariState* s, uint16_t addr, const uint8_t* rom_ptr, uint32_t rom_len) {
    uint8_t val = BR(s, addr);
    uint8_t old_carry = get_flag(s->status, FLAG_CARRY) ? 1 : 0;
    set_flag(&s->status, FLAG_CARRY, (val & 0x80) != 0);
    val = (val << 1) | old_carry;
    bus_write(s, addr, val);
    set_zn(&s->status, val);
}

__device__ __forceinline__
void do_ror_mem(AtariState* s, uint16_t addr, const uint8_t* rom_ptr, uint32_t rom_len) {
    uint8_t val = BR(s, addr);
    uint8_t old_carry = get_flag(s->status, FLAG_CARRY) ? 1 : 0;
    set_flag(&s->status, FLAG_CARRY, (val & 0x01) != 0);
    val = (val >> 1) | (old_carry << 7);
    bus_write(s, addr, val);
    set_zn(&s->status, val);
}

__device__ __forceinline__
void do_inc_mem(AtariState* s, uint16_t addr, const uint8_t* rom_ptr, uint32_t rom_len) {
    uint8_t val = BR(s, addr) + 1;
    bus_write(s, addr, val);
    set_zn(&s->status, val);
}

__device__ __forceinline__
void do_dec_mem(AtariState* s, uint16_t addr, const uint8_t* rom_ptr, uint32_t rom_len) {
    uint8_t val = BR(s, addr) - 1;
    bus_write(s, addr, val);
    set_zn(&s->status, val);
}

#define DO_ASL(s, addr) do_asl_mem(s, addr, rom_ptr, rom_len)
#define DO_LSR(s, addr) do_lsr_mem(s, addr, rom_ptr, rom_len)
#define DO_ROL(s, addr) do_rol_mem(s, addr, rom_ptr, rom_len)
#define DO_ROR(s, addr) do_ror_mem(s, addr, rom_ptr, rom_len)
#define DO_INC(s, addr) do_inc_mem(s, addr, rom_ptr, rom_len)
#define DO_DEC(s, addr) do_dec_mem(s, addr, rom_ptr, rom_len)

// =================================================================
// cpu_step: Execute one instruction. Returns cycles consumed.
// rom_ptr and rom_len must be in scope.
// =================================================================
__device__
uint8_t cpu_step(AtariState* s, const uint8_t* rom_ptr, uint32_t rom_len) {
    uint8_t opcode = BR(s, s->pc);
    uint8_t base_cycles = CYCLE_TABLE[opcode];
    uint8_t sz = SIZE_TABLE[opcode];
    uint8_t penalty = 0;

    switch (opcode) {

    // ======================== ADC ========================
    case 0x69: do_adc(s, IMM(s)); break;
    case 0x65: do_adc(s, BR(s, ADDR_ZP(s))); break;
    case 0x75: do_adc(s, BR(s, ADDR_ZPX(s))); break;
    case 0x6D: do_adc(s, BR(s, ADDR_ABS(s))); break;
    case 0x7D: do_adc(s, BR(s, ADDR_ABX(s))); penalty = PX_ABX(s); break;
    case 0x79: do_adc(s, BR(s, ADDR_ABY(s))); penalty = PX_ABY(s); break;
    case 0x61: do_adc(s, BR(s, ADDR_IZX(s))); break;
    case 0x71: do_adc(s, BR(s, ADDR_IZY(s))); penalty = PX_IZY(s); break;

    // ======================== SBC ========================
    case 0xE9: do_sbc(s, IMM(s)); break;
    case 0xE5: do_sbc(s, BR(s, ADDR_ZP(s))); break;
    case 0xF5: do_sbc(s, BR(s, ADDR_ZPX(s))); break;
    case 0xED: do_sbc(s, BR(s, ADDR_ABS(s))); break;
    case 0xFD: do_sbc(s, BR(s, ADDR_ABX(s))); penalty = PX_ABX(s); break;
    case 0xF9: do_sbc(s, BR(s, ADDR_ABY(s))); penalty = PX_ABY(s); break;
    case 0xE1: do_sbc(s, BR(s, ADDR_IZX(s))); break;
    case 0xF1: do_sbc(s, BR(s, ADDR_IZY(s))); penalty = PX_IZY(s); break;

    // ======================== AND ========================
    case 0x29: s->a &= IMM(s); set_zn(&s->status, s->a); break;
    case 0x25: s->a &= BR(s, ADDR_ZP(s)); set_zn(&s->status, s->a); break;
    case 0x35: s->a &= BR(s, ADDR_ZPX(s)); set_zn(&s->status, s->a); break;
    case 0x2D: s->a &= BR(s, ADDR_ABS(s)); set_zn(&s->status, s->a); break;
    case 0x3D: s->a &= BR(s, ADDR_ABX(s)); set_zn(&s->status, s->a); penalty = PX_ABX(s); break;
    case 0x39: s->a &= BR(s, ADDR_ABY(s)); set_zn(&s->status, s->a); penalty = PX_ABY(s); break;
    case 0x21: s->a &= BR(s, ADDR_IZX(s)); set_zn(&s->status, s->a); break;
    case 0x31: s->a &= BR(s, ADDR_IZY(s)); set_zn(&s->status, s->a); penalty = PX_IZY(s); break;

    // ======================== ORA ========================
    case 0x09: s->a |= IMM(s); set_zn(&s->status, s->a); break;
    case 0x05: s->a |= BR(s, ADDR_ZP(s)); set_zn(&s->status, s->a); break;
    case 0x15: s->a |= BR(s, ADDR_ZPX(s)); set_zn(&s->status, s->a); break;
    case 0x0D: s->a |= BR(s, ADDR_ABS(s)); set_zn(&s->status, s->a); break;
    case 0x1D: s->a |= BR(s, ADDR_ABX(s)); set_zn(&s->status, s->a); penalty = PX_ABX(s); break;
    case 0x19: s->a |= BR(s, ADDR_ABY(s)); set_zn(&s->status, s->a); penalty = PX_ABY(s); break;
    case 0x01: s->a |= BR(s, ADDR_IZX(s)); set_zn(&s->status, s->a); break;
    case 0x11: s->a |= BR(s, ADDR_IZY(s)); set_zn(&s->status, s->a); penalty = PX_IZY(s); break;

    // ======================== EOR ========================
    case 0x49: s->a ^= IMM(s); set_zn(&s->status, s->a); break;
    case 0x45: s->a ^= BR(s, ADDR_ZP(s)); set_zn(&s->status, s->a); break;
    case 0x55: s->a ^= BR(s, ADDR_ZPX(s)); set_zn(&s->status, s->a); break;
    case 0x4D: s->a ^= BR(s, ADDR_ABS(s)); set_zn(&s->status, s->a); break;
    case 0x5D: s->a ^= BR(s, ADDR_ABX(s)); set_zn(&s->status, s->a); penalty = PX_ABX(s); break;
    case 0x59: s->a ^= BR(s, ADDR_ABY(s)); set_zn(&s->status, s->a); penalty = PX_ABY(s); break;
    case 0x41: s->a ^= BR(s, ADDR_IZX(s)); set_zn(&s->status, s->a); break;
    case 0x51: s->a ^= BR(s, ADDR_IZY(s)); set_zn(&s->status, s->a); penalty = PX_IZY(s); break;

    // ======================== CMP ========================
    case 0xC9: do_cmp(&s->status, s->a, IMM(s)); break;
    case 0xC5: do_cmp(&s->status, s->a, BR(s, ADDR_ZP(s))); break;
    case 0xD5: do_cmp(&s->status, s->a, BR(s, ADDR_ZPX(s))); break;
    case 0xCD: do_cmp(&s->status, s->a, BR(s, ADDR_ABS(s))); break;
    case 0xDD: do_cmp(&s->status, s->a, BR(s, ADDR_ABX(s))); penalty = PX_ABX(s); break;
    case 0xD9: do_cmp(&s->status, s->a, BR(s, ADDR_ABY(s))); penalty = PX_ABY(s); break;
    case 0xC1: do_cmp(&s->status, s->a, BR(s, ADDR_IZX(s))); break;
    case 0xD1: do_cmp(&s->status, s->a, BR(s, ADDR_IZY(s))); penalty = PX_IZY(s); break;

    // ======================== CPX ========================
    case 0xE0: do_cmp(&s->status, s->x, IMM(s)); break;
    case 0xE4: do_cmp(&s->status, s->x, BR(s, ADDR_ZP(s))); break;
    case 0xEC: do_cmp(&s->status, s->x, BR(s, ADDR_ABS(s))); break;

    // ======================== CPY ========================
    case 0xC0: do_cmp(&s->status, s->y, IMM(s)); break;
    case 0xC4: do_cmp(&s->status, s->y, BR(s, ADDR_ZP(s))); break;
    case 0xCC: do_cmp(&s->status, s->y, BR(s, ADDR_ABS(s))); break;

    // ======================== LDA ========================
    case 0xA9: s->a = IMM(s); set_zn(&s->status, s->a); break;
    case 0xA5: s->a = BR(s, ADDR_ZP(s)); set_zn(&s->status, s->a); break;
    case 0xB5: s->a = BR(s, ADDR_ZPX(s)); set_zn(&s->status, s->a); break;
    case 0xAD: s->a = BR(s, ADDR_ABS(s)); set_zn(&s->status, s->a); break;
    case 0xBD: s->a = BR(s, ADDR_ABX(s)); set_zn(&s->status, s->a); penalty = PX_ABX(s); break;
    case 0xB9: s->a = BR(s, ADDR_ABY(s)); set_zn(&s->status, s->a); penalty = PX_ABY(s); break;
    case 0xA1: s->a = BR(s, ADDR_IZX(s)); set_zn(&s->status, s->a); break;
    case 0xB1: s->a = BR(s, ADDR_IZY(s)); set_zn(&s->status, s->a); penalty = PX_IZY(s); break;

    // ======================== LDX ========================
    case 0xA2: s->x = IMM(s); set_zn(&s->status, s->x); break;
    case 0xA6: s->x = BR(s, ADDR_ZP(s)); set_zn(&s->status, s->x); break;
    case 0xB6: s->x = BR(s, ADDR_ZPY(s)); set_zn(&s->status, s->x); break;
    case 0xAE: s->x = BR(s, ADDR_ABS(s)); set_zn(&s->status, s->x); break;
    case 0xBE: s->x = BR(s, ADDR_ABY(s)); set_zn(&s->status, s->x); penalty = PX_ABY(s); break;

    // ======================== LDY ========================
    case 0xA0: s->y = IMM(s); set_zn(&s->status, s->y); break;
    case 0xA4: s->y = BR(s, ADDR_ZP(s)); set_zn(&s->status, s->y); break;
    case 0xB4: s->y = BR(s, ADDR_ZPX(s)); set_zn(&s->status, s->y); break;
    case 0xAC: s->y = BR(s, ADDR_ABS(s)); set_zn(&s->status, s->y); break;
    case 0xBC: s->y = BR(s, ADDR_ABX(s)); set_zn(&s->status, s->y); penalty = PX_ABX(s); break;

    // ======================== STA ========================
    case 0x85: bus_write(s, ADDR_ZP(s), s->a); break;
    case 0x95: bus_write(s, ADDR_ZPX(s), s->a); break;
    case 0x8D: bus_write(s, ADDR_ABS(s), s->a); break;
    case 0x9D: bus_write(s, ADDR_ABX(s), s->a); break;
    case 0x99: bus_write(s, ADDR_ABY(s), s->a); break;
    case 0x81: bus_write(s, ADDR_IZX(s), s->a); break;
    case 0x91: bus_write(s, ADDR_IZY(s), s->a); break;

    // ======================== STX ========================
    case 0x86: bus_write(s, ADDR_ZP(s), s->x); break;
    case 0x96: bus_write(s, ADDR_ZPY(s), s->x); break;
    case 0x8E: bus_write(s, ADDR_ABS(s), s->x); break;

    // ======================== STY ========================
    case 0x84: bus_write(s, ADDR_ZP(s), s->y); break;
    case 0x94: bus_write(s, ADDR_ZPX(s), s->y); break;
    case 0x8C: bus_write(s, ADDR_ABS(s), s->y); break;

    // ======================== INC ========================
    case 0xE6: DO_INC(s, ADDR_ZP(s)); break;
    case 0xF6: DO_INC(s, ADDR_ZPX(s)); break;
    case 0xEE: DO_INC(s, ADDR_ABS(s)); break;
    case 0xFE: DO_INC(s, ADDR_ABX(s)); break;

    // ======================== DEC ========================
    case 0xC6: DO_DEC(s, ADDR_ZP(s)); break;
    case 0xD6: DO_DEC(s, ADDR_ZPX(s)); break;
    case 0xCE: DO_DEC(s, ADDR_ABS(s)); break;
    case 0xDE: DO_DEC(s, ADDR_ABX(s)); break;

    // ======================== INX/INY/DEX/DEY ========================
    case 0xE8: s->x += 1; set_zn(&s->status, s->x); break;
    case 0xC8: s->y += 1; set_zn(&s->status, s->y); break;
    case 0xCA: s->x -= 1; set_zn(&s->status, s->x); break;
    case 0x88: s->y -= 1; set_zn(&s->status, s->y); break;

    // ======================== ASL ========================
    case 0x0A:
        set_flag(&s->status, FLAG_CARRY, (s->a & 0x80) != 0);
        s->a <<= 1;
        set_zn(&s->status, s->a);
        break;
    case 0x06: DO_ASL(s, ADDR_ZP(s)); break;
    case 0x16: DO_ASL(s, ADDR_ZPX(s)); break;
    case 0x0E: DO_ASL(s, ADDR_ABS(s)); break;
    case 0x1E: DO_ASL(s, ADDR_ABX(s)); break;

    // ======================== LSR ========================
    case 0x4A:
        set_flag(&s->status, FLAG_CARRY, (s->a & 0x01) != 0);
        s->a >>= 1;
        set_flag(&s->status, FLAG_ZERO, s->a == 0);
        set_flag(&s->status, FLAG_NEGATIVE, false);
        break;
    case 0x46: DO_LSR(s, ADDR_ZP(s)); break;
    case 0x56: DO_LSR(s, ADDR_ZPX(s)); break;
    case 0x4E: DO_LSR(s, ADDR_ABS(s)); break;
    case 0x5E: DO_LSR(s, ADDR_ABX(s)); break;

    // ======================== ROL ========================
    case 0x2A: {
        uint8_t old_carry = get_flag(s->status, FLAG_CARRY) ? 1 : 0;
        set_flag(&s->status, FLAG_CARRY, (s->a & 0x80) != 0);
        s->a = (s->a << 1) | old_carry;
        set_zn(&s->status, s->a);
        break;
    }
    case 0x26: DO_ROL(s, ADDR_ZP(s)); break;
    case 0x36: DO_ROL(s, ADDR_ZPX(s)); break;
    case 0x2E: DO_ROL(s, ADDR_ABS(s)); break;
    case 0x3E: DO_ROL(s, ADDR_ABX(s)); break;

    // ======================== ROR ========================
    case 0x6A: {
        uint8_t old_carry = get_flag(s->status, FLAG_CARRY) ? 1 : 0;
        set_flag(&s->status, FLAG_CARRY, (s->a & 0x01) != 0);
        s->a = (s->a >> 1) | (old_carry << 7);
        set_zn(&s->status, s->a);
        break;
    }
    case 0x66: DO_ROR(s, ADDR_ZP(s)); break;
    case 0x76: DO_ROR(s, ADDR_ZPX(s)); break;
    case 0x6E: DO_ROR(s, ADDR_ABS(s)); break;
    case 0x7E: DO_ROR(s, ADDR_ABX(s)); break;

    // ======================== BIT ========================
    case 0x24: {
        uint8_t val = BR(s, ADDR_ZP(s));
        set_flag(&s->status, FLAG_ZERO, (s->a & val) == 0);
        set_flag(&s->status, FLAG_OVERFLOW, (val & 0x40) != 0);
        set_flag(&s->status, FLAG_NEGATIVE, (val & 0x80) != 0);
        break;
    }
    case 0x2C: {
        uint8_t val = BR(s, ADDR_ABS(s));
        set_flag(&s->status, FLAG_ZERO, (s->a & val) == 0);
        set_flag(&s->status, FLAG_OVERFLOW, (val & 0x40) != 0);
        set_flag(&s->status, FLAG_NEGATIVE, (val & 0x80) != 0);
        break;
    }

    // ======================== Transfers ========================
    case 0xAA: s->x = s->a; set_zn(&s->status, s->x); break;
    case 0x8A: s->a = s->x; set_zn(&s->status, s->a); break;
    case 0xA8: s->y = s->a; set_zn(&s->status, s->y); break;
    case 0x98: s->a = s->y; set_zn(&s->status, s->a); break;
    case 0x9A: s->sp = s->x; break;
    case 0xBA: s->x = s->sp; set_zn(&s->status, s->x); break;

    // ======================== Stack ========================
    case 0x48: push8(s, s->a); break;
    case 0x68: s->a = PULL8(s); set_zn(&s->status, s->a); break;
    case 0x08: push8(s, s->status | FLAG_BREAK | FLAG_UNUSED); break;
    case 0x28: {
        uint8_t flags = PULL8(s);
        s->status = flags & ~(FLAG_BREAK | FLAG_UNUSED);
        break;
    }

    // ======================== JMP ========================
    case 0x4C:
        s->pc = ADDR_ABS(s);
        return base_cycles;
    case 0x6C: {
        uint16_t ptr = R16(s, s->pc + 1);
        uint8_t lo = BR(s, ptr);
        uint16_t hi_addr = (ptr & 0xFF00) | ((ptr + 1) & 0x00FF);
        uint8_t hi = BR(s, hi_addr);
        s->pc = ((uint16_t)hi << 8) | lo;
        return base_cycles;
    }

    // ======================== JSR / RTS / RTI / BRK ========================
    case 0x20: {
        uint16_t target = ADDR_ABS(s);
        uint16_t ret = s->pc + 2;
        push16(s, ret);
        s->pc = target;
        return base_cycles;
    }
    case 0x60: {
        uint16_t ret = PULL16(s);
        s->pc = ret + 1;
        return base_cycles;
    }
    case 0x40: {
        uint8_t flags = PULL8(s);
        s->status = flags & ~(FLAG_BREAK | FLAG_UNUSED);
        s->pc = PULL16(s);
        return base_cycles;
    }
    case 0x00: {
        uint16_t ret = s->pc + 2;
        push16(s, ret);
        push8(s, s->status | FLAG_BREAK | FLAG_UNUSED);
        set_flag(&s->status, FLAG_INTERRUPT, true);
        s->pc = R16(s, 0xFFFE);
        return base_cycles;
    }

    // ======================== Flags ========================
    case 0x18: set_flag(&s->status, FLAG_CARRY, false); break;
    case 0x38: set_flag(&s->status, FLAG_CARRY, true); break;
    case 0x58: set_flag(&s->status, FLAG_INTERRUPT, false); break;
    case 0x78: set_flag(&s->status, FLAG_INTERRUPT, true); break;
    case 0xB8: set_flag(&s->status, FLAG_OVERFLOW, false); break;
    case 0xD8: set_flag(&s->status, FLAG_DECIMAL, false); break;
    case 0xF8: set_flag(&s->status, FLAG_DECIMAL, true); break;

    // ======================== Branches ========================
    case 0x10: return DO_BRANCH(s, !get_flag(s->status, FLAG_NEGATIVE), base_cycles);
    case 0x30: return DO_BRANCH(s,  get_flag(s->status, FLAG_NEGATIVE), base_cycles);
    case 0x50: return DO_BRANCH(s, !get_flag(s->status, FLAG_OVERFLOW), base_cycles);
    case 0x70: return DO_BRANCH(s,  get_flag(s->status, FLAG_OVERFLOW), base_cycles);
    case 0x90: return DO_BRANCH(s, !get_flag(s->status, FLAG_CARRY),    base_cycles);
    case 0xB0: return DO_BRANCH(s,  get_flag(s->status, FLAG_CARRY),    base_cycles);
    case 0xD0: return DO_BRANCH(s, !get_flag(s->status, FLAG_ZERO),     base_cycles);
    case 0xF0: return DO_BRANCH(s,  get_flag(s->status, FLAG_ZERO),     base_cycles);

    // ======================== NOP ========================
    case 0xEA: break;

    default: break;
    }

    s->pc += sz;
    return base_cycles + penalty;
}
