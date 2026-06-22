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

struct AffinePoint {
    U256 x;
    U256 y;
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
    for (uint i = 0; i < 8; i++) {
        out.v[i] = 0u;
    }
    return out;
}

inline U256 fe_one() {
    U256 out = fe_zero();
    out.v[0] = 1u;
    return out;
}

inline bool fe_is_zero(U256 a) {
    for (uint i = 0; i < 8; i++) {
        if (a.v[i] != 0u) {
            return false;
        }
    }
    return true;
}

inline void fold_single(thread U544& acc, uint h, uint offset) {
    if (h == 0u) {
        return;
    }

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

    if (borrow == 0u) {
        c = tmp;
    }
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
    for (uint i = 0; i < 16; i++) {
        acc.v[i] = tmp[i];
    }
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
    for (uint i = 0; i < 8; i++) {
        res.v[i] = acc.v[i];
    }
    return fe_cond_sub_p(res);
}

inline U256 fe_square(U256 a) { return fe_mul(a, a); }
inline U256 fe_double(U256 a) { return fe_add(a, a); }

inline U256 unpack_bigint(BigInt256 b) {
    U256 out = {};
    out.v[0] = b.v0.x;
    out.v[1] = b.v0.y;
    out.v[2] = b.v0.z;
    out.v[3] = b.v0.w;
    out.v[4] = b.v1.x;
    out.v[5] = b.v1.y;
    out.v[6] = b.v1.z;
    out.v[7] = b.v1.w;
    return out;
}

inline JacobianPoint jac_add_affine(JacobianPoint p, AffinePoint q) {
    if (fe_is_zero(q.x) && fe_is_zero(q.y)) {
        return p;
    }

    U256 u1 = p.x;
    U256 z1z1 = fe_square(p.z);
    U256 u2 = fe_mul(q.x, z1z1);
    U256 s1 = p.y;
    U256 z1z1z1 = fe_mul(z1z1, p.z);
    U256 s2 = fe_mul(q.y, z1z1z1);
    U256 h = fe_sub(u2, u1);
    U256 r = fe_double(fe_sub(s2, s1));

    if (fe_is_zero(h)) {
        return p;
    }

    U256 hh = fe_square(h);
    U256 i = fe_double(fe_double(hh));
    U256 j = fe_mul(h, i);
    U256 v = fe_mul(u1, i);
    U256 x3 = fe_sub(fe_sub(fe_square(r), j), fe_double(v));
    U256 y3 = fe_sub(fe_mul(r, fe_sub(v, x3)), fe_double(fe_mul(s1, j)));
    U256 z1_plus_1 = fe_add(p.z, fe_one());
    U256 z3 = fe_mul(fe_sub(fe_sub(fe_square(z1_plus_1), z1z1), fe_one()), h);

    JacobianPoint res = {};
    res.x = x3;
    res.y = y3;
    res.z = z3;
    return res;
}

kernel void compute_jacobian(
    constant Config& config [[buffer(0)]],
    device AffinePoint* table_rw [[buffer(1)]],
    device JacobianPoint* jacobian_points [[buffer(3)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= config.num_keys) {
        return;
    }

    JacobianPoint base_pub = {};
    base_pub.x = unpack_bigint(config.base_x);
    base_pub.y = unpack_bigint(config.base_y);
    base_pub.z = fe_one();

    AffinePoint point_i = table_rw[gid];
    jacobian_points[gid] = jac_add_affine(base_pub, point_i);
}
