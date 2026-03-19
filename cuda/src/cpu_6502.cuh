#pragma once
#include "state_layout.cuh"
#include "memory_bus.cuh"

#define FLAG_CARRY     (1 << 0)
#define FLAG_ZERO      (1 << 1)
#define FLAG_INTERRUPT (1 << 2)
#define FLAG_DECIMAL   (1 << 3)
#define FLAG_BREAK     (1 << 4)
#define FLAG_UNUSED    (1 << 5)
#define FLAG_OVERFLOW  (1 << 6)
#define FLAG_NEGATIVE  (1 << 7)

__device__ static const uint8_t CYCLE_TABLE[256] = {
    7,6,0,0,0,3,5,0,3,2,2,0,0,4,6,0, 2,5,0,0,0,4,6,0,2,4,0,0,0,4,7,0,
    6,6,0,0,3,3,5,0,4,2,2,0,4,4,6,0, 2,5,0,0,0,4,6,0,2,4,0,0,0,4,7,0,
    6,6,0,0,0,3,5,0,3,2,2,0,3,4,6,0, 2,5,0,0,0,4,6,0,2,4,0,0,0,4,7,0,
    6,6,0,0,0,3,5,0,4,2,2,0,5,4,6,0, 2,5,0,0,0,4,6,0,2,4,0,0,0,4,7,0,
    0,6,0,0,3,3,3,0,2,0,2,0,4,4,4,0, 2,6,0,0,4,4,4,0,2,5,2,0,5,5,0,0,
    2,6,2,0,3,3,3,0,2,2,2,0,4,4,4,0, 2,5,0,0,4,4,4,0,2,4,2,0,4,4,4,0,
    2,6,0,0,3,3,5,0,2,2,2,0,4,4,6,0, 2,5,0,0,0,4,6,0,2,4,0,0,0,4,7,0,
    2,6,0,0,3,3,5,0,2,2,2,0,4,4,6,0, 2,5,0,0,0,4,6,0,2,4,0,0,0,4,7,0,
};
__device__ static const uint8_t SIZE_TABLE[256] = {
    1,2,1,1,1,2,2,1,1,2,1,1,1,3,3,1, 2,2,1,1,1,2,2,1,1,3,1,1,1,3,3,1,
    3,2,1,1,2,2,2,1,1,2,1,1,3,3,3,1, 2,2,1,1,1,2,2,1,1,3,1,1,1,3,3,1,
    1,2,1,1,1,2,2,1,1,2,1,1,3,3,3,1, 2,2,1,1,1,2,2,1,1,3,1,1,1,3,3,1,
    1,2,1,1,1,2,2,1,1,2,1,1,3,3,3,1, 2,2,1,1,1,2,2,1,1,3,1,1,1,3,3,1,
    1,2,1,1,2,2,2,1,1,1,1,1,3,3,3,1, 2,2,1,1,2,2,2,1,1,3,1,1,3,3,1,1,
    2,2,2,1,2,2,2,1,1,2,1,1,3,3,3,1, 2,2,1,1,2,2,2,1,1,3,1,1,3,3,3,1,
    2,2,1,1,2,2,2,1,1,2,1,1,3,3,3,1, 2,2,1,1,1,2,2,1,1,3,1,1,1,3,3,1,
    2,2,1,1,2,2,2,1,1,2,1,1,3,3,3,1, 2,2,1,1,1,2,2,1,1,3,1,1,1,3,3,1,
};

// --- Helpers ---

__device__ __forceinline__
void set_flag(uint8_t* st, uint8_t f, bool on) { if (on) *st |= f; else *st &= ~f; }
__device__ __forceinline__
bool get_flag(uint8_t st, uint8_t f) { return (st & f) != 0; }
__device__ __forceinline__
void set_zn(uint8_t* st, uint8_t v) { set_flag(st, FLAG_ZERO, v==0); set_flag(st, FLAG_NEGATIVE, (v&0x80)!=0); }

// All functions take: ThreadCtx* c, uint8_t* mr (my_ram in shared),
//                     const uint8_t* rp (rom_ptr), uint32_t rl (rom_len)
// Macros thread these implicitly.

#define BR(addr)       bus_read(c, mr, addr, rp, rl)
#define BW(addr, val)  bus_write(c, mr, addr, val)
#define R16(addr)      (((uint16_t)BR((uint16_t)((addr)+1)) << 8) | BR(addr))

// Stack
#define PUSH8(val)     do { BW(0x0100 | (uint16_t)c->sp, val); c->sp--; } while(0)
#define PULL8_VAL()    (c->sp++, BR(0x0100 | (uint16_t)c->sp))
#define PUSH16(val)    do { PUSH8((uint8_t)((val)>>8)); PUSH8((uint8_t)((val)&0xFF)); } while(0)
#define PULL16_VAL()   __pull16(c, mr, rp, rl)

__device__ __forceinline__
uint16_t __pull16(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl) {
    uint8_t lo = PULL8_VAL();
    uint8_t hi = PULL8_VAL();
    return ((uint16_t)hi << 8) | lo;
}

// Address modes
#define A_ZP()   ((uint16_t)BR(c->pc+1))
#define A_ZPX()  ((uint16_t)((BR(c->pc+1)+c->x)&0xFF))
#define A_ZPY()  ((uint16_t)((BR(c->pc+1)+c->y)&0xFF))
#define A_ABS()  R16(c->pc+1)
#define A_ABX()  ((uint16_t)(R16(c->pc+1)+c->x))
#define A_ABY()  ((uint16_t)(R16(c->pc+1)+c->y))
#define A_IZX()  __addr_izx(c, mr, rp, rl)
#define A_IZY()  __addr_izy(c, mr, rp, rl)
#define IMM()    BR(c->pc+1)

__device__ __forceinline__
uint16_t __addr_izx(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl) {
    uint8_t p = BR(c->pc+1) + c->x;
    return ((uint16_t)BR((uint16_t)((p+1)&0xFF)) << 8) | BR((uint16_t)p);
}
__device__ __forceinline__
uint16_t __addr_izy(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl) {
    uint8_t z = BR(c->pc+1);
    uint16_t base = ((uint16_t)BR((uint16_t)((z+1)&0xFF)) << 8) | BR((uint16_t)z);
    return base + c->y;
}

// Page cross penalties
#define PX_ABX() __px_abx(c, mr, rp, rl)
#define PX_ABY() __px_aby(c, mr, rp, rl)
#define PX_IZY() __px_izy(c, mr, rp, rl)

__device__ __forceinline__
uint8_t __px_abx(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl) {
    uint16_t b = R16(c->pc+1); return ((b&0xFF00)!=((b+c->x)&0xFF00))?1:0;
}
__device__ __forceinline__
uint8_t __px_aby(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl) {
    uint16_t b = R16(c->pc+1); return ((b&0xFF00)!=((b+c->y)&0xFF00))?1:0;
}
__device__ __forceinline__
uint8_t __px_izy(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl) {
    uint8_t z = BR(c->pc+1);
    uint16_t b = ((uint16_t)BR((uint16_t)((z+1)&0xFF))<<8) | BR((uint16_t)z);
    return ((b&0xFF00)!=((b+c->y)&0xFF00))?1:0;
}

// ADC
__device__ __forceinline__
void do_adc(ThreadCtx* c, uint8_t val) {
    uint8_t carry = get_flag(c->status, FLAG_CARRY)?1:0;
    if (get_flag(c->status, FLAG_DECIMAL)) {
        uint8_t lo = (c->a&0x0F)+(val&0x0F)+carry; if (lo>9) lo+=6;
        uint8_t hi = (c->a>>4)+(val>>4)+(lo>0x0F?1:0);
        uint16_t bs = (uint16_t)c->a+(uint16_t)val+(uint16_t)carry;
        set_flag(&c->status, FLAG_ZERO, (uint8_t)bs==0);
        set_flag(&c->status, FLAG_NEGATIVE, (hi&0x08)!=0);
        set_flag(&c->status, FLAG_OVERFLOW, ((~(c->a^val)&(c->a^((hi<<4)|(lo&0x0F))))&0x80)!=0);
        if (hi>9) hi+=6;
        set_flag(&c->status, FLAG_CARRY, hi>0x0F);
        c->a = ((hi&0x0F)<<4)|(lo&0x0F);
    } else {
        uint16_t s = (uint16_t)c->a+(uint16_t)val+(uint16_t)carry;
        set_flag(&c->status, FLAG_CARRY, s>0xFF);
        set_flag(&c->status, FLAG_OVERFLOW, ((~(c->a^val)&(c->a^(uint8_t)s))&0x80)!=0);
        c->a = (uint8_t)s; set_zn(&c->status, c->a);
    }
}

// SBC
__device__ __forceinline__
void do_sbc(ThreadCtx* c, uint8_t val) {
    uint8_t borrow = get_flag(c->status, FLAG_CARRY)?0:1;
    if (get_flag(c->status, FLAG_DECIMAL)) {
        uint8_t lo = (c->a&0x0F)-(val&0x0F)-borrow;
        uint8_t lb = ((int8_t)lo<0)?1:0; if(lb) lo-=6;
        uint8_t hi = (c->a>>4)-(val>>4)-lb; if((int8_t)hi<0) hi-=6;
        int16_t bd = (int16_t)c->a-(int16_t)val-(int16_t)borrow;
        set_flag(&c->status, FLAG_CARRY, bd>=0);
        set_flag(&c->status, FLAG_ZERO, (uint8_t)bd==0);
        set_flag(&c->status, FLAG_NEGATIVE, ((uint8_t)bd&0x80)!=0);
        set_flag(&c->status, FLAG_OVERFLOW, (((c->a^val)&(c->a^(uint8_t)bd))&0x80)!=0);
        c->a = ((hi&0x0F)<<4)|(lo&0x0F);
    } else {
        uint16_t d = (uint16_t)c->a-(uint16_t)val-(uint16_t)borrow;
        set_flag(&c->status, FLAG_CARRY, d<0x100);
        set_flag(&c->status, FLAG_OVERFLOW, (((c->a^val)&(c->a^(uint8_t)d))&0x80)!=0);
        c->a = (uint8_t)d; set_zn(&c->status, c->a);
    }
}

__device__ __forceinline__
void do_cmp(uint8_t* st, uint8_t reg, uint8_t val) {
    set_flag(st, FLAG_CARRY, reg>=val); set_zn(st, reg-val);
}

// Branch
__device__ __forceinline__
uint8_t do_branch(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl,
                  bool taken, uint8_t bc) {
    if (!taken) { c->pc += 2; return bc; }
    uint8_t off = BR(c->pc+1);
    uint16_t npc = c->pc+2;
    uint16_t tgt = npc + (uint16_t)(int16_t)(int8_t)off;
    c->pc = tgt;
    return bc + 1 + (((npc&0xFF00)!=(tgt&0xFF00))?1:0);
}
#define DO_BR(taken, bc) do_branch(c, mr, rp, rl, taken, bc)

// RMW helpers
#define RMW_BODY(op) { uint8_t v = BR(a); op; BW(a, v); }

__device__ __forceinline__
void do_asl_m(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl, uint16_t a) {
    uint8_t v=BR(a); set_flag(&c->status,FLAG_CARRY,(v&0x80)!=0); v<<=1; BW(a,v); set_zn(&c->status,v);
}
__device__ __forceinline__
void do_lsr_m(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl, uint16_t a) {
    uint8_t v=BR(a); set_flag(&c->status,FLAG_CARRY,(v&0x01)!=0); v>>=1; BW(a,v);
    set_flag(&c->status,FLAG_ZERO,v==0); set_flag(&c->status,FLAG_NEGATIVE,false);
}
__device__ __forceinline__
void do_rol_m(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl, uint16_t a) {
    uint8_t v=BR(a); uint8_t oc=get_flag(c->status,FLAG_CARRY)?1:0;
    set_flag(&c->status,FLAG_CARRY,(v&0x80)!=0); v=(v<<1)|oc; BW(a,v); set_zn(&c->status,v);
}
__device__ __forceinline__
void do_ror_m(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl, uint16_t a) {
    uint8_t v=BR(a); uint8_t oc=get_flag(c->status,FLAG_CARRY)?1:0;
    set_flag(&c->status,FLAG_CARRY,(v&0x01)!=0); v=(v>>1)|(oc<<7); BW(a,v); set_zn(&c->status,v);
}
__device__ __forceinline__
void do_inc_m(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl, uint16_t a) {
    uint8_t v=BR(a)+1; BW(a,v); set_zn(&c->status,v);
}
__device__ __forceinline__
void do_dec_m(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl, uint16_t a) {
    uint8_t v=BR(a)-1; BW(a,v); set_zn(&c->status,v);
}
#define ASL_M(a) do_asl_m(c,mr,rp,rl,a)
#define LSR_M(a) do_lsr_m(c,mr,rp,rl,a)
#define ROL_M(a) do_rol_m(c,mr,rp,rl,a)
#define ROR_M(a) do_ror_m(c,mr,rp,rl,a)
#define INC_M(a) do_inc_m(c,mr,rp,rl,a)
#define DEC_M(a) do_dec_m(c,mr,rp,rl,a)

// =================================================================
// cpu_step: Execute one instruction. Returns cycles consumed.
// =================================================================
__device__
uint8_t cpu_step(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl) {
    uint8_t op = BR(c->pc);
    uint8_t bc = CYCLE_TABLE[op];
    uint8_t sz = SIZE_TABLE[op];
    uint8_t pen = 0;

    switch (op) {
    // ADC
    case 0x69: do_adc(c,IMM()); break;
    case 0x65: do_adc(c,BR(A_ZP())); break;
    case 0x75: do_adc(c,BR(A_ZPX())); break;
    case 0x6D: do_adc(c,BR(A_ABS())); break;
    case 0x7D: do_adc(c,BR(A_ABX())); pen=PX_ABX(); break;
    case 0x79: do_adc(c,BR(A_ABY())); pen=PX_ABY(); break;
    case 0x61: do_adc(c,BR(A_IZX())); break;
    case 0x71: do_adc(c,BR(A_IZY())); pen=PX_IZY(); break;
    // SBC
    case 0xE9: do_sbc(c,IMM()); break;
    case 0xE5: do_sbc(c,BR(A_ZP())); break;
    case 0xF5: do_sbc(c,BR(A_ZPX())); break;
    case 0xED: do_sbc(c,BR(A_ABS())); break;
    case 0xFD: do_sbc(c,BR(A_ABX())); pen=PX_ABX(); break;
    case 0xF9: do_sbc(c,BR(A_ABY())); pen=PX_ABY(); break;
    case 0xE1: do_sbc(c,BR(A_IZX())); break;
    case 0xF1: do_sbc(c,BR(A_IZY())); pen=PX_IZY(); break;
    // AND
    case 0x29: c->a&=IMM(); set_zn(&c->status,c->a); break;
    case 0x25: c->a&=BR(A_ZP()); set_zn(&c->status,c->a); break;
    case 0x35: c->a&=BR(A_ZPX()); set_zn(&c->status,c->a); break;
    case 0x2D: c->a&=BR(A_ABS()); set_zn(&c->status,c->a); break;
    case 0x3D: c->a&=BR(A_ABX()); set_zn(&c->status,c->a); pen=PX_ABX(); break;
    case 0x39: c->a&=BR(A_ABY()); set_zn(&c->status,c->a); pen=PX_ABY(); break;
    case 0x21: c->a&=BR(A_IZX()); set_zn(&c->status,c->a); break;
    case 0x31: c->a&=BR(A_IZY()); set_zn(&c->status,c->a); pen=PX_IZY(); break;
    // ORA
    case 0x09: c->a|=IMM(); set_zn(&c->status,c->a); break;
    case 0x05: c->a|=BR(A_ZP()); set_zn(&c->status,c->a); break;
    case 0x15: c->a|=BR(A_ZPX()); set_zn(&c->status,c->a); break;
    case 0x0D: c->a|=BR(A_ABS()); set_zn(&c->status,c->a); break;
    case 0x1D: c->a|=BR(A_ABX()); set_zn(&c->status,c->a); pen=PX_ABX(); break;
    case 0x19: c->a|=BR(A_ABY()); set_zn(&c->status,c->a); pen=PX_ABY(); break;
    case 0x01: c->a|=BR(A_IZX()); set_zn(&c->status,c->a); break;
    case 0x11: c->a|=BR(A_IZY()); set_zn(&c->status,c->a); pen=PX_IZY(); break;
    // EOR
    case 0x49: c->a^=IMM(); set_zn(&c->status,c->a); break;
    case 0x45: c->a^=BR(A_ZP()); set_zn(&c->status,c->a); break;
    case 0x55: c->a^=BR(A_ZPX()); set_zn(&c->status,c->a); break;
    case 0x4D: c->a^=BR(A_ABS()); set_zn(&c->status,c->a); break;
    case 0x5D: c->a^=BR(A_ABX()); set_zn(&c->status,c->a); pen=PX_ABX(); break;
    case 0x59: c->a^=BR(A_ABY()); set_zn(&c->status,c->a); pen=PX_ABY(); break;
    case 0x41: c->a^=BR(A_IZX()); set_zn(&c->status,c->a); break;
    case 0x51: c->a^=BR(A_IZY()); set_zn(&c->status,c->a); pen=PX_IZY(); break;
    // CMP
    case 0xC9: do_cmp(&c->status,c->a,IMM()); break;
    case 0xC5: do_cmp(&c->status,c->a,BR(A_ZP())); break;
    case 0xD5: do_cmp(&c->status,c->a,BR(A_ZPX())); break;
    case 0xCD: do_cmp(&c->status,c->a,BR(A_ABS())); break;
    case 0xDD: do_cmp(&c->status,c->a,BR(A_ABX())); pen=PX_ABX(); break;
    case 0xD9: do_cmp(&c->status,c->a,BR(A_ABY())); pen=PX_ABY(); break;
    case 0xC1: do_cmp(&c->status,c->a,BR(A_IZX())); break;
    case 0xD1: do_cmp(&c->status,c->a,BR(A_IZY())); pen=PX_IZY(); break;
    // CPX
    case 0xE0: do_cmp(&c->status,c->x,IMM()); break;
    case 0xE4: do_cmp(&c->status,c->x,BR(A_ZP())); break;
    case 0xEC: do_cmp(&c->status,c->x,BR(A_ABS())); break;
    // CPY
    case 0xC0: do_cmp(&c->status,c->y,IMM()); break;
    case 0xC4: do_cmp(&c->status,c->y,BR(A_ZP())); break;
    case 0xCC: do_cmp(&c->status,c->y,BR(A_ABS())); break;
    // LDA
    case 0xA9: c->a=IMM(); set_zn(&c->status,c->a); break;
    case 0xA5: c->a=BR(A_ZP()); set_zn(&c->status,c->a); break;
    case 0xB5: c->a=BR(A_ZPX()); set_zn(&c->status,c->a); break;
    case 0xAD: c->a=BR(A_ABS()); set_zn(&c->status,c->a); break;
    case 0xBD: c->a=BR(A_ABX()); set_zn(&c->status,c->a); pen=PX_ABX(); break;
    case 0xB9: c->a=BR(A_ABY()); set_zn(&c->status,c->a); pen=PX_ABY(); break;
    case 0xA1: c->a=BR(A_IZX()); set_zn(&c->status,c->a); break;
    case 0xB1: c->a=BR(A_IZY()); set_zn(&c->status,c->a); pen=PX_IZY(); break;
    // LDX
    case 0xA2: c->x=IMM(); set_zn(&c->status,c->x); break;
    case 0xA6: c->x=BR(A_ZP()); set_zn(&c->status,c->x); break;
    case 0xB6: c->x=BR(A_ZPY()); set_zn(&c->status,c->x); break;
    case 0xAE: c->x=BR(A_ABS()); set_zn(&c->status,c->x); break;
    case 0xBE: c->x=BR(A_ABY()); set_zn(&c->status,c->x); pen=PX_ABY(); break;
    // LDY
    case 0xA0: c->y=IMM(); set_zn(&c->status,c->y); break;
    case 0xA4: c->y=BR(A_ZP()); set_zn(&c->status,c->y); break;
    case 0xB4: c->y=BR(A_ZPX()); set_zn(&c->status,c->y); break;
    case 0xAC: c->y=BR(A_ABS()); set_zn(&c->status,c->y); break;
    case 0xBC: c->y=BR(A_ABX()); set_zn(&c->status,c->y); pen=PX_ABX(); break;
    // STA
    case 0x85: BW(A_ZP(),c->a); break;
    case 0x95: BW(A_ZPX(),c->a); break;
    case 0x8D: BW(A_ABS(),c->a); break;
    case 0x9D: BW(A_ABX(),c->a); break;
    case 0x99: BW(A_ABY(),c->a); break;
    case 0x81: BW(A_IZX(),c->a); break;
    case 0x91: BW(A_IZY(),c->a); break;
    // STX
    case 0x86: BW(A_ZP(),c->x); break;
    case 0x96: BW(A_ZPY(),c->x); break;
    case 0x8E: BW(A_ABS(),c->x); break;
    // STY
    case 0x84: BW(A_ZP(),c->y); break;
    case 0x94: BW(A_ZPX(),c->y); break;
    case 0x8C: BW(A_ABS(),c->y); break;
    // INC/DEC
    case 0xE6: INC_M(A_ZP()); break;  case 0xF6: INC_M(A_ZPX()); break;
    case 0xEE: INC_M(A_ABS()); break; case 0xFE: INC_M(A_ABX()); break;
    case 0xC6: DEC_M(A_ZP()); break;  case 0xD6: DEC_M(A_ZPX()); break;
    case 0xCE: DEC_M(A_ABS()); break; case 0xDE: DEC_M(A_ABX()); break;
    // INX/INY/DEX/DEY
    case 0xE8: c->x+=1; set_zn(&c->status,c->x); break;
    case 0xC8: c->y+=1; set_zn(&c->status,c->y); break;
    case 0xCA: c->x-=1; set_zn(&c->status,c->x); break;
    case 0x88: c->y-=1; set_zn(&c->status,c->y); break;
    // ASL
    case 0x0A: set_flag(&c->status,FLAG_CARRY,(c->a&0x80)!=0); c->a<<=1; set_zn(&c->status,c->a); break;
    case 0x06: ASL_M(A_ZP()); break;  case 0x16: ASL_M(A_ZPX()); break;
    case 0x0E: ASL_M(A_ABS()); break; case 0x1E: ASL_M(A_ABX()); break;
    // LSR
    case 0x4A: set_flag(&c->status,FLAG_CARRY,(c->a&0x01)!=0); c->a>>=1;
        set_flag(&c->status,FLAG_ZERO,c->a==0); set_flag(&c->status,FLAG_NEGATIVE,false); break;
    case 0x46: LSR_M(A_ZP()); break;  case 0x56: LSR_M(A_ZPX()); break;
    case 0x4E: LSR_M(A_ABS()); break; case 0x5E: LSR_M(A_ABX()); break;
    // ROL
    case 0x2A: { uint8_t oc=get_flag(c->status,FLAG_CARRY)?1:0;
        set_flag(&c->status,FLAG_CARRY,(c->a&0x80)!=0); c->a=(c->a<<1)|oc; set_zn(&c->status,c->a); break; }
    case 0x26: ROL_M(A_ZP()); break;  case 0x36: ROL_M(A_ZPX()); break;
    case 0x2E: ROL_M(A_ABS()); break; case 0x3E: ROL_M(A_ABX()); break;
    // ROR
    case 0x6A: { uint8_t oc=get_flag(c->status,FLAG_CARRY)?1:0;
        set_flag(&c->status,FLAG_CARRY,(c->a&0x01)!=0); c->a=(c->a>>1)|(oc<<7); set_zn(&c->status,c->a); break; }
    case 0x66: ROR_M(A_ZP()); break;  case 0x76: ROR_M(A_ZPX()); break;
    case 0x6E: ROR_M(A_ABS()); break; case 0x7E: ROR_M(A_ABX()); break;
    // BIT
    case 0x24: { uint8_t v=BR(A_ZP()); set_flag(&c->status,FLAG_ZERO,(c->a&v)==0);
        set_flag(&c->status,FLAG_OVERFLOW,(v&0x40)!=0); set_flag(&c->status,FLAG_NEGATIVE,(v&0x80)!=0); break; }
    case 0x2C: { uint8_t v=BR(A_ABS()); set_flag(&c->status,FLAG_ZERO,(c->a&v)==0);
        set_flag(&c->status,FLAG_OVERFLOW,(v&0x40)!=0); set_flag(&c->status,FLAG_NEGATIVE,(v&0x80)!=0); break; }
    // Transfers
    case 0xAA: c->x=c->a; set_zn(&c->status,c->x); break;
    case 0x8A: c->a=c->x; set_zn(&c->status,c->a); break;
    case 0xA8: c->y=c->a; set_zn(&c->status,c->y); break;
    case 0x98: c->a=c->y; set_zn(&c->status,c->a); break;
    case 0x9A: c->sp=c->x; break;
    case 0xBA: c->x=c->sp; set_zn(&c->status,c->x); break;
    // Stack
    case 0x48: PUSH8(c->a); break;
    case 0x68: c->a=PULL8_VAL(); set_zn(&c->status,c->a); break;
    case 0x08: PUSH8(c->status|FLAG_BREAK|FLAG_UNUSED); break;
    case 0x28: { uint8_t f=PULL8_VAL(); c->status=f&~(FLAG_BREAK|FLAG_UNUSED); break; }
    // JMP
    case 0x4C: c->pc=A_ABS(); return bc;
    case 0x6C: { uint16_t p=R16(c->pc+1); uint8_t lo=BR(p);
        uint8_t hi=BR((p&0xFF00)|((p+1)&0x00FF)); c->pc=((uint16_t)hi<<8)|lo; return bc; }
    // JSR/RTS/RTI/BRK
    case 0x20: { uint16_t t=A_ABS(); PUSH16(c->pc+2); c->pc=t; return bc; }
    case 0x60: { uint16_t r=PULL16_VAL(); c->pc=r+1; return bc; }
    case 0x40: { uint8_t f=PULL8_VAL(); c->status=f&~(FLAG_BREAK|FLAG_UNUSED); c->pc=PULL16_VAL(); return bc; }
    case 0x00: { PUSH16(c->pc+2); PUSH8(c->status|FLAG_BREAK|FLAG_UNUSED);
        set_flag(&c->status,FLAG_INTERRUPT,true); c->pc=R16(0xFFFE); return bc; }
    // Flags
    case 0x18: set_flag(&c->status,FLAG_CARRY,false); break;
    case 0x38: set_flag(&c->status,FLAG_CARRY,true); break;
    case 0x58: set_flag(&c->status,FLAG_INTERRUPT,false); break;
    case 0x78: set_flag(&c->status,FLAG_INTERRUPT,true); break;
    case 0xB8: set_flag(&c->status,FLAG_OVERFLOW,false); break;
    case 0xD8: set_flag(&c->status,FLAG_DECIMAL,false); break;
    case 0xF8: set_flag(&c->status,FLAG_DECIMAL,true); break;
    // Branches
    case 0x10: return DO_BR(!get_flag(c->status,FLAG_NEGATIVE),bc);
    case 0x30: return DO_BR( get_flag(c->status,FLAG_NEGATIVE),bc);
    case 0x50: return DO_BR(!get_flag(c->status,FLAG_OVERFLOW),bc);
    case 0x70: return DO_BR( get_flag(c->status,FLAG_OVERFLOW),bc);
    case 0x90: return DO_BR(!get_flag(c->status,FLAG_CARRY),bc);
    case 0xB0: return DO_BR( get_flag(c->status,FLAG_CARRY),bc);
    case 0xD0: return DO_BR(!get_flag(c->status,FLAG_ZERO),bc);
    case 0xF0: return DO_BR( get_flag(c->status,FLAG_ZERO),bc);
    // NOP
    case 0xEA: break;
    default: break;
    }
    c->pc += sz;
    return bc + pen;
}
