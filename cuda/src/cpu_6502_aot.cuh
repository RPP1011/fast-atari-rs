#pragma once
#include "state_layout.cuh"
#include "memory_bus.cuh"
#include "decoded_op.cuh"

// AOT CPU step: uses pre-decoded ROM table instead of bus_read for opcode/operand fetch.
// Eliminates ~30 branches per instruction (3 bus_read calls × ~10 branches each).
//
// What changes vs cpu_6502.cuh:
//   - Opcode + operands come from DecodedOp (fetched once at top via __ldg)
//   - IMM/ZP/ZPX/ZPY/ABS/ABX/ABY address modes use d.operand directly
//   - Page cross penalties computed from d.operand (no re-read)
//   - IZX/IZY: zero-page base address from d.operand, pointer deref still via BR()
//   - Branches: offset from (int8_t)d.operand
//   - JMP indirect: pointer deref still via BR() (could target RAM)
//   - Bankswitch hotspot fallback: if PC is in $xFF4-$xFFF, use original cpu_step

// Flag constants reused from cpu_6502.cuh (already defined via #include)

// --- AOT address mode macros ---
// These replace the bus_read-based macros from cpu_6502.cuh.
// d is the DecodedOp for the current instruction.

#define AOT_IMM()    ((uint8_t)d.operand)
#define AOT_ZP()     ((uint16_t)(uint8_t)d.operand)
#define AOT_ZPX()    ((uint16_t)(((uint8_t)d.operand + c->x) & 0xFF))
#define AOT_ZPY()    ((uint16_t)(((uint8_t)d.operand + c->y) & 0xFF))
#define AOT_ABS()    (d.operand)
#define AOT_ABX()    ((uint16_t)(d.operand + c->x))
#define AOT_ABY()    ((uint16_t)(d.operand + c->y))

// IZX: base ZP address from operand, pointer deref via bus_read
__device__ __forceinline__
uint16_t __aot_addr_izx(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl, uint16_t operand) {
    uint8_t p = (uint8_t)operand + c->x;
    return ((uint16_t)BR((uint16_t)((p+1)&0xFF)) << 8) | BR((uint16_t)p);
}

// IZY: base ZP address from operand, pointer deref via bus_read
__device__ __forceinline__
uint16_t __aot_addr_izy(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl, uint16_t operand) {
    uint8_t z = (uint8_t)operand;
    uint16_t base = ((uint16_t)BR((uint16_t)((z+1)&0xFF)) << 8) | BR((uint16_t)z);
    return base + c->y;
}

#define AOT_IZX() __aot_addr_izx(c, mr, rp, rl, d.operand)
#define AOT_IZY() __aot_addr_izy(c, mr, rp, rl, d.operand)

// Page cross penalties — computed from pre-decoded operand (no bus_read)
#define AOT_PX_ABX() (((d.operand & 0xFF00) != ((d.operand + c->x) & 0xFF00)) ? 1 : 0)
#define AOT_PX_ABY() (((d.operand & 0xFF00) != ((d.operand + c->y) & 0xFF00)) ? 1 : 0)

__device__ __forceinline__
uint8_t __aot_px_izy(ThreadCtx* c, uint8_t* mr, const uint8_t* rp, uint32_t rl, uint16_t operand) {
    uint8_t z = (uint8_t)operand;
    uint16_t b = ((uint16_t)BR((uint16_t)((z+1)&0xFF))<<8) | BR((uint16_t)z);
    return ((b&0xFF00)!=((b+c->y)&0xFF00))?1:0;
}
#define AOT_PX_IZY() __aot_px_izy(c, mr, rp, rl, d.operand)

// Branch helper using pre-decoded operand as offset
__device__ __forceinline__
uint8_t do_branch_aot(ThreadCtx* c, bool taken, uint8_t bc, uint16_t operand) {
    if (!taken) { c->pc += 2; return bc; }
    uint16_t npc = c->pc + 2;
    uint16_t tgt = npc + (uint16_t)(int16_t)(int8_t)(uint8_t)operand;
    c->pc = tgt;
    return bc + 1 + (((npc&0xFF00)!=(tgt&0xFF00))?1:0);
}
#define AOT_BR(taken, bc) do_branch_aot(c, taken, bc, d.operand)

// RMW helpers — same as cpu_6502.cuh, just using AOT address macros
#define AOT_ASL_M(a) do_asl_m(c,mr,rp,rl,a)
#define AOT_LSR_M(a) do_lsr_m(c,mr,rp,rl,a)
#define AOT_ROL_M(a) do_rol_m(c,mr,rp,rl,a)
#define AOT_ROR_M(a) do_ror_m(c,mr,rp,rl,a)
#define AOT_INC_M(a) do_inc_m(c,mr,rp,rl,a)
#define AOT_DEC_M(a) do_dec_m(c,mr,rp,rl,a)

// =================================================================
// cpu_step_aot: Execute one instruction using pre-decoded ROM table.
// Falls back to cpu_step for bankswitch hotspot addresses.
// =================================================================
__device__
uint8_t cpu_step_aot(ThreadCtx* c, uint8_t* mr,
                     const uint8_t* rp, uint32_t rl,
                     const DecodedOp* __restrict__ decode_table) {
    // Bankswitch hotspot guard: fall back for PCs near hotspot region.
    // F4 uses $1FF4-$1FFB, so guard everything >= $xFF4.
    if (__builtin_expect((c->pc & 0x0FFF) >= 0x0FF4, 0))
        return cpu_step(c, mr, rp, rl);

    DecodedOp d = decode_fetch(decode_table, c->bank, c->pc);
    uint8_t bc = d.cycles;
    uint8_t sz = d.size;
    uint8_t pen = 0;

    switch (d.opcode) {
    // ADC
    case 0x69: do_adc(c,AOT_IMM()); break;
    case 0x65: do_adc(c,BR(AOT_ZP())); break;
    case 0x75: do_adc(c,BR(AOT_ZPX())); break;
    case 0x6D: do_adc(c,BR(AOT_ABS())); break;
    case 0x7D: do_adc(c,BR(AOT_ABX())); pen=AOT_PX_ABX(); break;
    case 0x79: do_adc(c,BR(AOT_ABY())); pen=AOT_PX_ABY(); break;
    case 0x61: do_adc(c,BR(AOT_IZX())); break;
    case 0x71: do_adc(c,BR(AOT_IZY())); pen=AOT_PX_IZY(); break;
    // SBC
    case 0xE9: do_sbc(c,AOT_IMM()); break;
    case 0xE5: do_sbc(c,BR(AOT_ZP())); break;
    case 0xF5: do_sbc(c,BR(AOT_ZPX())); break;
    case 0xED: do_sbc(c,BR(AOT_ABS())); break;
    case 0xFD: do_sbc(c,BR(AOT_ABX())); pen=AOT_PX_ABX(); break;
    case 0xF9: do_sbc(c,BR(AOT_ABY())); pen=AOT_PX_ABY(); break;
    case 0xE1: do_sbc(c,BR(AOT_IZX())); break;
    case 0xF1: do_sbc(c,BR(AOT_IZY())); pen=AOT_PX_IZY(); break;
    // AND
    case 0x29: c->a&=AOT_IMM(); set_zn(&c->status,c->a); break;
    case 0x25: c->a&=BR(AOT_ZP()); set_zn(&c->status,c->a); break;
    case 0x35: c->a&=BR(AOT_ZPX()); set_zn(&c->status,c->a); break;
    case 0x2D: c->a&=BR(AOT_ABS()); set_zn(&c->status,c->a); break;
    case 0x3D: c->a&=BR(AOT_ABX()); set_zn(&c->status,c->a); pen=AOT_PX_ABX(); break;
    case 0x39: c->a&=BR(AOT_ABY()); set_zn(&c->status,c->a); pen=AOT_PX_ABY(); break;
    case 0x21: c->a&=BR(AOT_IZX()); set_zn(&c->status,c->a); break;
    case 0x31: c->a&=BR(AOT_IZY()); set_zn(&c->status,c->a); pen=AOT_PX_IZY(); break;
    // ORA
    case 0x09: c->a|=AOT_IMM(); set_zn(&c->status,c->a); break;
    case 0x05: c->a|=BR(AOT_ZP()); set_zn(&c->status,c->a); break;
    case 0x15: c->a|=BR(AOT_ZPX()); set_zn(&c->status,c->a); break;
    case 0x0D: c->a|=BR(AOT_ABS()); set_zn(&c->status,c->a); break;
    case 0x1D: c->a|=BR(AOT_ABX()); set_zn(&c->status,c->a); pen=AOT_PX_ABX(); break;
    case 0x19: c->a|=BR(AOT_ABY()); set_zn(&c->status,c->a); pen=AOT_PX_ABY(); break;
    case 0x01: c->a|=BR(AOT_IZX()); set_zn(&c->status,c->a); break;
    case 0x11: c->a|=BR(AOT_IZY()); set_zn(&c->status,c->a); pen=AOT_PX_IZY(); break;
    // EOR
    case 0x49: c->a^=AOT_IMM(); set_zn(&c->status,c->a); break;
    case 0x45: c->a^=BR(AOT_ZP()); set_zn(&c->status,c->a); break;
    case 0x55: c->a^=BR(AOT_ZPX()); set_zn(&c->status,c->a); break;
    case 0x4D: c->a^=BR(AOT_ABS()); set_zn(&c->status,c->a); break;
    case 0x5D: c->a^=BR(AOT_ABX()); set_zn(&c->status,c->a); pen=AOT_PX_ABX(); break;
    case 0x59: c->a^=BR(AOT_ABY()); set_zn(&c->status,c->a); pen=AOT_PX_ABY(); break;
    case 0x41: c->a^=BR(AOT_IZX()); set_zn(&c->status,c->a); break;
    case 0x51: c->a^=BR(AOT_IZY()); set_zn(&c->status,c->a); pen=AOT_PX_IZY(); break;
    // CMP
    case 0xC9: do_cmp(&c->status,c->a,AOT_IMM()); break;
    case 0xC5: do_cmp(&c->status,c->a,BR(AOT_ZP())); break;
    case 0xD5: do_cmp(&c->status,c->a,BR(AOT_ZPX())); break;
    case 0xCD: do_cmp(&c->status,c->a,BR(AOT_ABS())); break;
    case 0xDD: do_cmp(&c->status,c->a,BR(AOT_ABX())); pen=AOT_PX_ABX(); break;
    case 0xD9: do_cmp(&c->status,c->a,BR(AOT_ABY())); pen=AOT_PX_ABY(); break;
    case 0xC1: do_cmp(&c->status,c->a,BR(AOT_IZX())); break;
    case 0xD1: do_cmp(&c->status,c->a,BR(AOT_IZY())); pen=AOT_PX_IZY(); break;
    // CPX
    case 0xE0: do_cmp(&c->status,c->x,AOT_IMM()); break;
    case 0xE4: do_cmp(&c->status,c->x,BR(AOT_ZP())); break;
    case 0xEC: do_cmp(&c->status,c->x,BR(AOT_ABS())); break;
    // CPY
    case 0xC0: do_cmp(&c->status,c->y,AOT_IMM()); break;
    case 0xC4: do_cmp(&c->status,c->y,BR(AOT_ZP())); break;
    case 0xCC: do_cmp(&c->status,c->y,BR(AOT_ABS())); break;
    // LDA
    case 0xA9: c->a=AOT_IMM(); set_zn(&c->status,c->a); break;
    case 0xA5: c->a=BR(AOT_ZP()); set_zn(&c->status,c->a); break;
    case 0xB5: c->a=BR(AOT_ZPX()); set_zn(&c->status,c->a); break;
    case 0xAD: c->a=BR(AOT_ABS()); set_zn(&c->status,c->a); break;
    case 0xBD: c->a=BR(AOT_ABX()); set_zn(&c->status,c->a); pen=AOT_PX_ABX(); break;
    case 0xB9: c->a=BR(AOT_ABY()); set_zn(&c->status,c->a); pen=AOT_PX_ABY(); break;
    case 0xA1: c->a=BR(AOT_IZX()); set_zn(&c->status,c->a); break;
    case 0xB1: c->a=BR(AOT_IZY()); set_zn(&c->status,c->a); pen=AOT_PX_IZY(); break;
    // LDX
    case 0xA2: c->x=AOT_IMM(); set_zn(&c->status,c->x); break;
    case 0xA6: c->x=BR(AOT_ZP()); set_zn(&c->status,c->x); break;
    case 0xB6: c->x=BR(AOT_ZPY()); set_zn(&c->status,c->x); break;
    case 0xAE: c->x=BR(AOT_ABS()); set_zn(&c->status,c->x); break;
    case 0xBE: c->x=BR(AOT_ABY()); set_zn(&c->status,c->x); pen=AOT_PX_ABY(); break;
    // LDY
    case 0xA0: c->y=AOT_IMM(); set_zn(&c->status,c->y); break;
    case 0xA4: c->y=BR(AOT_ZP()); set_zn(&c->status,c->y); break;
    case 0xB4: c->y=BR(AOT_ZPX()); set_zn(&c->status,c->y); break;
    case 0xAC: c->y=BR(AOT_ABS()); set_zn(&c->status,c->y); break;
    case 0xBC: c->y=BR(AOT_ABX()); set_zn(&c->status,c->y); pen=AOT_PX_ABX(); break;
    // STA
    case 0x85: BW(AOT_ZP(),c->a); break;
    case 0x95: BW(AOT_ZPX(),c->a); break;
    case 0x8D: BW(AOT_ABS(),c->a); break;
    case 0x9D: BW(AOT_ABX(),c->a); break;
    case 0x99: BW(AOT_ABY(),c->a); break;
    case 0x81: BW(AOT_IZX(),c->a); break;
    case 0x91: BW(AOT_IZY(),c->a); break;
    // STX
    case 0x86: BW(AOT_ZP(),c->x); break;
    case 0x96: BW(AOT_ZPY(),c->x); break;
    case 0x8E: BW(AOT_ABS(),c->x); break;
    // STY
    case 0x84: BW(AOT_ZP(),c->y); break;
    case 0x94: BW(AOT_ZPX(),c->y); break;
    case 0x8C: BW(AOT_ABS(),c->y); break;
    // INC/DEC
    case 0xE6: AOT_INC_M(AOT_ZP()); break;  case 0xF6: AOT_INC_M(AOT_ZPX()); break;
    case 0xEE: AOT_INC_M(AOT_ABS()); break; case 0xFE: AOT_INC_M(AOT_ABX()); break;
    case 0xC6: AOT_DEC_M(AOT_ZP()); break;  case 0xD6: AOT_DEC_M(AOT_ZPX()); break;
    case 0xCE: AOT_DEC_M(AOT_ABS()); break; case 0xDE: AOT_DEC_M(AOT_ABX()); break;
    // INX/INY/DEX/DEY
    case 0xE8: c->x+=1; set_zn(&c->status,c->x); break;
    case 0xC8: c->y+=1; set_zn(&c->status,c->y); break;
    case 0xCA: c->x-=1; set_zn(&c->status,c->x); break;
    case 0x88: c->y-=1; set_zn(&c->status,c->y); break;
    // ASL
    case 0x0A: set_flag(&c->status,FLAG_CARRY,(c->a&0x80)!=0); c->a<<=1; set_zn(&c->status,c->a); break;
    case 0x06: AOT_ASL_M(AOT_ZP()); break;  case 0x16: AOT_ASL_M(AOT_ZPX()); break;
    case 0x0E: AOT_ASL_M(AOT_ABS()); break; case 0x1E: AOT_ASL_M(AOT_ABX()); break;
    // LSR
    case 0x4A: set_flag(&c->status,FLAG_CARRY,(c->a&0x01)!=0); c->a>>=1;
        set_flag(&c->status,FLAG_ZERO,c->a==0); set_flag(&c->status,FLAG_NEGATIVE,false); break;
    case 0x46: AOT_LSR_M(AOT_ZP()); break;  case 0x56: AOT_LSR_M(AOT_ZPX()); break;
    case 0x4E: AOT_LSR_M(AOT_ABS()); break; case 0x5E: AOT_LSR_M(AOT_ABX()); break;
    // ROL
    case 0x2A: { uint8_t oc=get_flag(c->status,FLAG_CARRY)?1:0;
        set_flag(&c->status,FLAG_CARRY,(c->a&0x80)!=0); c->a=(c->a<<1)|oc; set_zn(&c->status,c->a); break; }
    case 0x26: AOT_ROL_M(AOT_ZP()); break;  case 0x36: AOT_ROL_M(AOT_ZPX()); break;
    case 0x2E: AOT_ROL_M(AOT_ABS()); break; case 0x3E: AOT_ROL_M(AOT_ABX()); break;
    // ROR
    case 0x6A: { uint8_t oc=get_flag(c->status,FLAG_CARRY)?1:0;
        set_flag(&c->status,FLAG_CARRY,(c->a&0x01)!=0); c->a=(c->a>>1)|(oc<<7); set_zn(&c->status,c->a); break; }
    case 0x66: AOT_ROR_M(AOT_ZP()); break;  case 0x76: AOT_ROR_M(AOT_ZPX()); break;
    case 0x6E: AOT_ROR_M(AOT_ABS()); break; case 0x7E: AOT_ROR_M(AOT_ABX()); break;
    // BIT
    case 0x24: { uint8_t v=BR(AOT_ZP()); set_flag(&c->status,FLAG_ZERO,(c->a&v)==0);
        set_flag(&c->status,FLAG_OVERFLOW,(v&0x40)!=0); set_flag(&c->status,FLAG_NEGATIVE,(v&0x80)!=0); break; }
    case 0x2C: { uint8_t v=BR(AOT_ABS()); set_flag(&c->status,FLAG_ZERO,(c->a&v)==0);
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
    // JMP absolute — target from pre-decoded operand
    case 0x4C: c->pc=AOT_ABS(); return bc;
    // JMP indirect — pointer deref via bus_read (could be in RAM)
    case 0x6C: { uint16_t p=AOT_ABS(); uint8_t lo=BR(p);
        uint8_t hi=BR((p&0xFF00)|((p+1)&0x00FF)); c->pc=((uint16_t)hi<<8)|lo; return bc; }
    // JSR/RTS/RTI/BRK
    case 0x20: { uint16_t t=AOT_ABS(); PUSH16(c->pc+2); c->pc=t; return bc; }
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
    // Branches — offset from pre-decoded operand
    case 0x10: return AOT_BR(!get_flag(c->status,FLAG_NEGATIVE),bc);
    case 0x30: return AOT_BR( get_flag(c->status,FLAG_NEGATIVE),bc);
    case 0x50: return AOT_BR(!get_flag(c->status,FLAG_OVERFLOW),bc);
    case 0x70: return AOT_BR( get_flag(c->status,FLAG_OVERFLOW),bc);
    case 0x90: return AOT_BR(!get_flag(c->status,FLAG_CARRY),bc);
    case 0xB0: return AOT_BR( get_flag(c->status,FLAG_CARRY),bc);
    case 0xD0: return AOT_BR(!get_flag(c->status,FLAG_ZERO),bc);
    case 0xF0: return AOT_BR( get_flag(c->status,FLAG_ZERO),bc);
    // NOP
    case 0xEA: break;
    default: break;
    }
    c->pc += sz;
    return bc + pen;
}
