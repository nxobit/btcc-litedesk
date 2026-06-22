#include <metal_stdlib>
using namespace metal;

constant uint P0 = 0xFFFFFC2Fu;
constant uint P1 = 0xFFFFFFFEu;
constant uint P2 = 0xFFFFFFFFu;
constant uint P3 = 0xFFFFFFFFu;
constant uint P4 = 0xFFFFFFFFu;
constant uint P5 = 0xFFFFFFFFu;
constant uint P6 = 0xFFFFFFFFu;
constant uint P7 = 0xFFFFFFFFu;

struct U256 {
    uint v[8];
};

struct U544 {
    uint v[17];
};

struct JacobianPoint {
    U256 x;
    U256 y;
    U256 z;
};

struct BigInt256 {
    uint4 v0;
    uint4 v1;
};

struct PatternConfig {
    uint match_mode;
    uint prefix_len;
    uint suffix_len;
    uint _pad;
    uint4 prefix_chars[11];
    uint4 suffix_chars[11];
};

struct Config {
    BigInt256 base_x;
    BigInt256 base_y;
    uint num_keys;
    uint _pad0;
    uint _pad1;
    uint _pad2;
    PatternConfig pattern;
};

inline uint2 mul32(uint a, uint b) {
    ulong product = (ulong)a * (ulong)b;
    return uint2((uint)(product & 0xfffffffful), (uint)(product >> 32u));
}

inline U256 fe_zero() {
    U256 out = {};
    for (uint i = 0; i < 8; i++) out.v[i] = 0u;
    return out;
}

inline U256 fe_one() {
    U256 out = fe_zero();
    out.v[0] = 1u;
    return out;
}

inline bool fe_is_zero(U256 a) {
    for (uint i = 0; i < 8; i++) {
        if (a.v[i] != 0u) return false;
    }
    return true;
}

inline void fold_single(thread U544& acc, uint h, uint offset) {
    if (h == 0u) return;

    uint2 t = mul32(h, 977u);
    uint sum = acc.v[offset] + t.x;
    uint carry = (sum < acc.v[offset]) ? 1u : 0u;
    acc.v[offset] = sum;

    sum = acc.v[offset + 1u] + t.y + carry;
    carry = (sum < acc.v[offset + 1u] || (carry == 1u && sum == acc.v[offset + 1u])) ? 1u : 0u;
    acc.v[offset + 1u] = sum;

    sum = acc.v[offset + 1u] + h;
    carry = carry + ((sum < acc.v[offset + 1u]) ? 1u : 0u);
    acc.v[offset + 1u] = sum;

    uint k = offset + 2u;
    while (carry != 0u && k < 17u) {
        sum = acc.v[k] + carry;
        carry = (sum < acc.v[k]) ? 1u : 0u;
        acc.v[k] = sum;
        k++;
    }
}

inline U256 fe_cond_sub_p(U256 val) {
    U256 c = val;
    U256 tmp = {};
    uint borrow = 0u;
    uint diff = 0u;

    diff = c.v[0] - P0; borrow = (c.v[0] < P0) ? 1u : 0u; tmp.v[0] = diff;
    diff = c.v[1] - P1 - borrow; borrow = (c.v[1] < P1 + borrow) ? 1u : 0u; tmp.v[1] = diff;
    diff = c.v[2] - P2 - borrow; borrow = (c.v[2] < P2 + borrow) ? 1u : 0u; tmp.v[2] = diff;
    diff = c.v[3] - P3 - borrow; borrow = (c.v[3] < P3 + borrow) ? 1u : 0u; tmp.v[3] = diff;
    diff = c.v[4] - P4 - borrow; borrow = (c.v[4] < P4 + borrow) ? 1u : 0u; tmp.v[4] = diff;
    diff = c.v[5] - P5 - borrow; borrow = (c.v[5] < P5 + borrow) ? 1u : 0u; tmp.v[5] = diff;
    diff = c.v[6] - P6 - borrow; borrow = (c.v[6] < P6 + borrow) ? 1u : 0u; tmp.v[6] = diff;
    diff = c.v[7] - P7 - borrow; borrow = (c.v[7] < P7 + borrow) ? 1u : 0u; tmp.v[7] = diff;

    if (borrow == 0u) c = tmp;
    return c;
}

inline U256 fe_add(U256 a, U256 b) {
    U256 c = {};
    uint carry = 0u;
    for (uint i = 0; i < 8; i++) {
        uint sum = a.v[i] + b.v[i] + carry;
        carry = (sum < a.v[i] || (carry == 1u && sum == a.v[i])) ? 1u : 0u;
        c.v[i] = sum;
    }
    if (carry == 1u) {
        uint old = c.v[0];
        c.v[0] = c.v[0] + 977u;
        carry = (c.v[0] < old) ? 1u : 0u;
        old = c.v[1];
        c.v[1] = c.v[1] + 1u + carry;
        carry = (c.v[1] < old || (carry == 1u && c.v[1] == old)) ? 1u : 0u;
        for (uint i = 2; i < 8; i++) {
            old = c.v[i];
            c.v[i] = c.v[i] + carry;
            carry = (c.v[i] < old) ? 1u : 0u;
        }
    }
    return fe_cond_sub_p(c);
}

inline U256 fe_sub(U256 a, U256 b) {
    U256 c = {};
    uint borrow = 0u;
    for (uint i = 0; i < 8; i++) {
        uint subtrahend = b.v[i] + borrow;
        uint diff = a.v[i] - subtrahend;
        borrow = (a.v[i] < subtrahend) ? 1u : 0u;
        c.v[i] = diff;
    }

    if (borrow == 1u) {
        const uint p[8] = {P0, P1, P2, P3, P4, P5, P6, P7};
        uint carry = 0u;
        for (uint i = 0; i < 8; i++) {
            uint old = c.v[i];
            c.v[i] = c.v[i] + p[i] + carry;
            carry = (c.v[i] < old || (carry == 1u && c.v[i] == old)) ? 1u : 0u;
        }
    }

    return c;
}

inline U256 fe_mul(U256 a, U256 b) {
    uint tmp[16] = {};
    for (uint i = 0; i < 8; i++) {
        uint2 carry = uint2(0u);
        for (uint j = 0; j < 8; j++) {
            uint2 t = mul32(a.v[i], b.v[j]);
            uint k = i + j;
            uint acc = tmp[k] + t.x;
            uint c = (acc < tmp[k]) ? 1u : 0u;
            tmp[k] = acc;
            acc = tmp[k + 1u] + t.y + c;
            c = (acc < tmp[k + 1u] || (c == 1u && acc == tmp[k + 1u])) ? 1u : 0u;
            acc = acc + carry.x;
            c = c + ((acc < carry.x) ? 1u : 0u);
            tmp[k + 1u] = acc;
            carry = uint2(carry.y + c, 0u);
        }
        tmp[i + 8u] = tmp[i + 8u] + carry.x;
    }

    U544 acc = {};
    for (uint i = 0; i < 16; i++) acc.v[i] = tmp[i];
    acc.v[16] = 0u;

    for (uint i = 8; i < 16; i++) {
        uint h = acc.v[i];
        acc.v[i] = 0u;
        fold_single(acc, h, i - 8u);
        fold_single(acc, h, i - 7u);
    }

    uint h16 = acc.v[16];
    acc.v[16] = 0u;
    fold_single(acc, h16, 8u);
    fold_single(acc, h16, 9u);

    U256 res = {};
    for (uint i = 0; i < 8; i++) res.v[i] = acc.v[i];
    return fe_cond_sub_p(res);
}

inline U256 fe_square(U256 a) { return fe_mul(a, a); }

inline U256 fe_inv(U256 a) {
    U256 res = fe_one();
    U256 base = a;
    const uint exp[8] = {
        0xFFFFFC2Du, 0xFFFFFFFEu, 0xFFFFFFFFu, 0xFFFFFFFFu,
        0xFFFFFFFFu, 0xFFFFFFFFu, 0xFFFFFFFFu, 0xFFFFFFFFu
    };
    for (uint i = 0; i < 8; i++) {
        uint limb = exp[i];
        for (uint j = 0; j < 32; j++) {
            if (((limb >> j) & 1u) == 1u) res = fe_mul(res, base);
            base = fe_square(base);
        }
    }
    return res;
}

inline uint pattern_char(constant uint4* chunks, uint index) {
    uint4 chunk = chunks[index / 4u];
    uint offset = index % 4u;
    return offset == 0u ? chunk.x : (offset == 1u ? chunk.y : (offset == 2u ? chunk.z : chunk.w));
}

inline uint bech32_charset(uint value) {
    const uint chars[32] = {
        113u, 112u, 122u, 114u, 121u, 57u, 120u, 56u,
        103u, 102u, 50u, 116u, 118u, 100u, 119u, 48u,
        115u, 51u, 106u, 110u, 53u, 52u, 107u, 104u,
        99u, 101u, 54u, 109u, 117u, 97u, 55u, 108u
    };
    return chars[value];
}

inline uint hash160_byte(const uint words[5], uint index) {
    uint word = words[index / 4u];
    uint shift = (index % 4u) * 8u;
    return (word >> shift) & 0xffu;
}

inline uint bech32_polymod_step(uint pre, uint value) {
    uint b = pre >> 25u;
    uint chk = ((pre & 0x1ffffffu) << 5u) ^ value;
    if ((b & 1u) != 0u) chk ^= 0x3b6a57b2u;
    if ((b & 2u) != 0u) chk ^= 0x26508e6du;
    if ((b & 4u) != 0u) chk ^= 0x1ea119fau;
    if ((b & 8u) != 0u) chk ^= 0x3d4233ddu;
    if ((b & 16u) != 0u) chk ^= 0x2a1462b3u;
    return chk;
}

inline bool match_btcc_address(constant Config& config, const uint chars[42]) {
    if (config.pattern.match_mode == 1u || config.pattern.match_mode == 3u) {
        for (uint i = 0u; i < config.pattern.prefix_len; i++) {
            if (chars[i] != pattern_char(config.pattern.prefix_chars, i)) return false;
        }
    }
    if (config.pattern.match_mode == 2u || config.pattern.match_mode == 3u) {
        for (uint i = 0u; i < config.pattern.suffix_len; i++) {
            uint addr_idx = 42u - config.pattern.suffix_len + i;
            if (chars[addr_idx] != pattern_char(config.pattern.suffix_chars, i)) return false;
        }
    }
    return true;
}

inline uint rrot(uint x, uint n) { return (x >> n) | (x << (32u - n)); }
inline uint ch(uint x, uint y, uint z) { return (x & y) ^ ((~x) & z); }
inline uint maj(uint x, uint y, uint z) { return (x & y) ^ (x & z) ^ (y & z); }
inline uint sigma0(uint x) { return rrot(x, 2u) ^ rrot(x, 13u) ^ rrot(x, 22u); }
inline uint sigma1(uint x) { return rrot(x, 6u) ^ rrot(x, 11u) ^ rrot(x, 25u); }
inline uint gamma0(uint x) { return rrot(x, 7u) ^ rrot(x, 18u) ^ (x >> 3u); }
inline uint gamma1(uint x) { return rrot(x, 17u) ^ rrot(x, 19u) ^ (x >> 10u); }

inline void sha256_compressed_pubkey(uint parity, U256 x, thread uint out[8]) {
    uint h[8] = {
        0x6a09e667u, 0xbb67ae85u, 0x3c6ef372u, 0xa54ff53au,
        0x510e527fu, 0x9b05688cu, 0x1f83d9abu, 0x5be0cd19u
    };
    uint w[64] = {};
    uint prefix = ((parity & 1u) == 1u) ? 0x03000000u : 0x02000000u;
    w[0] = prefix | (x.v[7] >> 8u);
    w[1] = (x.v[7] << 24u) | (x.v[6] >> 8u);
    w[2] = (x.v[6] << 24u) | (x.v[5] >> 8u);
    w[3] = (x.v[5] << 24u) | (x.v[4] >> 8u);
    w[4] = (x.v[4] << 24u) | (x.v[3] >> 8u);
    w[5] = (x.v[3] << 24u) | (x.v[2] >> 8u);
    w[6] = (x.v[2] << 24u) | (x.v[1] >> 8u);
    w[7] = (x.v[1] << 24u) | (x.v[0] >> 8u);
    w[8] = (x.v[0] << 24u) | 0x00800000u;
    w[15] = 264u;
    for (uint i = 16u; i < 64u; i++) {
        w[i] = w[i - 16u] + gamma0(w[i - 15u]) + w[i - 7u] + gamma1(w[i - 2u]);
    }
    uint a = h[0], b = h[1], c = h[2], d = h[3], e = h[4], f = h[5], g = h[6], hv = h[7];
    const uint k[64] = {
        0x428a2f98u, 0x71374491u, 0xb5c0fbcfu, 0xe9b5dba5u, 0x3956c25bu, 0x59f111f1u, 0x923f82a4u, 0xab1c5ed5u,
        0xd807aa98u, 0x12835b01u, 0x243185beu, 0x550c7dc3u, 0x72be5d74u, 0x80deb1feu, 0x9bdc06a7u, 0xc19bf174u,
        0xe49b69c1u, 0xefbe4786u, 0x0fc19dc6u, 0x240ca1ccu, 0x2de92c6fu, 0x4a7484aau, 0x5cb0a9dcu, 0x76f988dau,
        0x983e5152u, 0xa831c66du, 0xb00327c8u, 0xbf597fc7u, 0xc6e00bf3u, 0xd5a79147u, 0x06ca6351u, 0x14292967u,
        0x27b70a85u, 0x2e1b2138u, 0x4d2c6dfcu, 0x53380d13u, 0x650a7354u, 0x766a0abbu, 0x81c2c92eu, 0x92722c85u,
        0xa2bfe8a1u, 0xa81a664bu, 0xc24b8b70u, 0xc76c51a3u, 0xd192e819u, 0xd6990624u, 0xf40e3585u, 0x106aa070u,
        0x19a4c116u, 0x1e376c08u, 0x2748774cu, 0x34b0bcb5u, 0x391c0cb3u, 0x4ed8aa4au, 0x5b9cca4fu, 0x682e6ff3u,
        0x748f82eeu, 0x78a5636fu, 0x84c87814u, 0x8cc70208u, 0x90befffau, 0xa4506cebu, 0xbef9a3f7u, 0xc67178f2u
    };
    for (uint i = 0u; i < 64u; i++) {
        uint t1 = hv + sigma1(e) + ch(e, f, g) + k[i] + w[i];
        uint t2 = sigma0(a) + maj(a, b, c);
        hv = g; g = f; f = e; e = d + t1; d = c; c = b; b = a; a = t1 + t2;
    }
    out[0] = h[0] + a; out[1] = h[1] + b; out[2] = h[2] + c; out[3] = h[3] + d;
    out[4] = h[4] + e; out[5] = h[5] + f; out[6] = h[6] + g; out[7] = h[7] + hv;
}

inline uint rol(uint x, uint n) { return (x << n) | (x >> (32u - n)); }

inline void ripemd160(const thread uint sha_out[8], thread uint out[5]) {
    uint x[16] = {};
    for (uint i = 0u; i < 8u; i++) {
        uint w = sha_out[i];
        x[i] = ((w & 0xFFu) << 24u) | ((w & 0xFF00u) << 8u) | ((w & 0xFF0000u) >> 8u) | (w >> 24u);
    }
    x[8] = 0x00000080u;
    x[14] = 256u;
    const uint rl[80] = {
        0u,1u,2u,3u,4u,5u,6u,7u,8u,9u,10u,11u,12u,13u,14u,15u,
        7u,4u,13u,1u,10u,6u,15u,3u,12u,0u,9u,5u,2u,14u,11u,8u,
        3u,10u,14u,4u,9u,15u,8u,1u,2u,7u,0u,6u,13u,11u,5u,12u,
        1u,9u,11u,10u,0u,8u,12u,4u,13u,3u,7u,15u,14u,5u,6u,2u,
        4u,0u,5u,9u,7u,12u,2u,10u,14u,1u,3u,8u,11u,6u,15u,13u
    };
    const uint rr[80] = {
        5u,14u,7u,0u,9u,2u,11u,4u,13u,6u,15u,8u,1u,10u,3u,12u,
        6u,11u,3u,7u,0u,13u,5u,10u,14u,15u,8u,12u,4u,9u,1u,2u,
        15u,5u,1u,3u,7u,14u,6u,9u,11u,8u,12u,2u,10u,0u,4u,13u,
        8u,6u,4u,1u,3u,11u,15u,0u,5u,12u,2u,13u,9u,7u,10u,14u,
        12u,15u,10u,4u,1u,5u,8u,7u,6u,2u,13u,14u,0u,3u,9u,11u
    };
    const uint sl[80] = {
        11u,14u,15u,12u,5u,8u,7u,9u,11u,13u,14u,15u,6u,7u,9u,8u,
        7u,6u,8u,13u,11u,9u,7u,15u,7u,12u,15u,9u,11u,7u,13u,12u,
        11u,13u,6u,7u,14u,9u,13u,15u,14u,8u,13u,6u,5u,12u,7u,5u,
        11u,12u,14u,15u,14u,15u,9u,8u,9u,14u,5u,6u,8u,6u,5u,12u,
        9u,15u,5u,11u,6u,8u,13u,12u,5u,12u,13u,14u,11u,8u,5u,6u
    };
    const uint sr[80] = {
        8u,9u,9u,11u,13u,15u,15u,5u,7u,7u,8u,11u,14u,14u,12u,6u,
        9u,13u,15u,7u,12u,8u,9u,11u,7u,7u,12u,7u,6u,15u,13u,11u,
        9u,7u,15u,11u,8u,6u,6u,14u,12u,13u,5u,14u,13u,13u,7u,5u,
        15u,5u,8u,11u,14u,14u,6u,14u,6u,9u,12u,9u,12u,5u,15u,8u,
        8u,5u,12u,9u,12u,5u,14u,6u,8u,13u,6u,5u,15u,13u,11u,11u
    };
    const uint kl[5] = {0x00000000u, 0x5a827999u, 0x6ed9eba1u, 0x8f1bbcdcu, 0xa953fd4eu};
    const uint kr[5] = {0x50a28be6u, 0x5c4dd124u, 0x6d703ef3u, 0x7a6d76e9u, 0x00000000u};

    uint al = 0x67452301u, bl = 0xefcdab89u, cl = 0x98badcfeu, dl = 0x10325476u, el = 0xc3d2e1f0u;
    uint ar = al, br = bl, cr = cl, dr = dl, er = el;

    for (uint i = 0u; i < 80u; i++) {
        uint f_left;
        if (i < 16u) f_left = bl ^ cl ^ dl;
        else if (i < 32u) f_left = (bl & cl) | ((~bl) & dl);
        else if (i < 48u) f_left = (bl | (~cl)) ^ dl;
        else if (i < 64u) f_left = (bl & dl) | (cl & (~dl));
        else f_left = bl ^ (cl | (~dl));

        uint t_left = rol(al + f_left + x[rl[i]] + kl[i / 16u], sl[i]) + el;
        al = el; el = dl; dl = rol(cl, 10u); cl = bl; bl = t_left;

        uint f_right;
        if (i < 16u) f_right = br ^ (cr | (~dr));
        else if (i < 32u) f_right = (br & dr) | (cr & (~dr));
        else if (i < 48u) f_right = (br | (~cr)) ^ dr;
        else if (i < 64u) f_right = (br & cr) | ((~br) & dr);
        else f_right = br ^ cr ^ dr;

        uint t_right = rol(ar + f_right + x[rr[i]] + kr[i / 16u], sr[i]) + er;
        ar = er; er = dr; dr = rol(cr, 10u); cr = br; br = t_right;
    }

    out[0] = 0xefcdab89u + cl + dr;
    out[1] = 0x98badcfeu + dl + er;
    out[2] = 0x10325476u + el + ar;
    out[3] = 0xc3d2e1f0u + al + br;
    out[4] = 0x67452301u + bl + cr;
}

inline void build_btcc_address_chars(const thread uint hash160[5], thread uint chars[42]) {
    uint data[33] = {};
    data[0] = 0u;
    uint acc = 0u;
    uint bits = 0u;
    uint data_idx = 1u;

    for (uint i = 0u; i < 20u; i++) {
        acc = (acc << 8u) | hash160_byte(hash160, i);
        bits += 8u;
        while (bits >= 5u) {
            bits -= 5u;
            data[data_idx++] = (acc >> bits) & 31u;
        }
    }
    if (bits > 0u) data[data_idx++] = (acc << (5u - bits)) & 31u;

    uint checksum[6] = {};
    uint chk = 1u;
    chk = bech32_polymod_step(chk, 3u);
    chk = bech32_polymod_step(chk, 3u);
    chk = bech32_polymod_step(chk, 0u);
    chk = bech32_polymod_step(chk, 3u);
    chk = bech32_polymod_step(chk, 3u);
    for (uint i = 0u; i < 33u; i++) chk = bech32_polymod_step(chk, data[i]);
    for (uint i = 0u; i < 6u; i++) chk = bech32_polymod_step(chk, 0u);
    uint polymod = chk ^ 1u;
    for (uint i = 0u; i < 6u; i++) checksum[i] = (polymod >> (5u * (5u - i))) & 31u;

    chars[0] = 99u; chars[1] = 99u; chars[2] = 49u;
    for (uint i = 0u; i < 33u; i++) chars[3u + i] = bech32_charset(data[i]);
    for (uint i = 0u; i < 6u; i++) chars[36u + i] = bech32_charset(checksum[i]);
}

kernel void batch_normalize_btcc_match(
    constant Config& config [[buffer(0)]],
    device JacobianPoint* jacobian_points [[buffer(3)]],
    device atomic_uint* match_result [[buffer(5)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= config.num_keys) return;

    JacobianPoint p = jacobian_points[gid];
    U256 z_inv = fe_inv(p.z);
    U256 z_inv2 = fe_square(z_inv);
    U256 z_inv3 = fe_mul(z_inv2, z_inv);
    U256 x = fe_mul(p.x, z_inv2);
    U256 y = fe_mul(p.y, z_inv3);

    uint parity = y.v[0] & 1u;
    uint sha_out[8] = {};
    uint ripemd_out[5] = {};
    uint chars[42] = {};

    sha256_compressed_pubkey(parity, x, sha_out);
    ripemd160(sha_out, ripemd_out);
    build_btcc_address_chars(ripemd_out, chars);

    if (match_btcc_address(config, chars)) {
        atomic_fetch_min_explicit(&match_result[0], gid, memory_order_relaxed);
    }
}
