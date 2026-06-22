const P0: u32 = 0xFFFFFC2Fu;
const P1: u32 = 0xFFFFFFFEu;
const P2: u32 = 0xFFFFFFFFu;
const P3: u32 = 0xFFFFFFFFu;
const P4: u32 = 0xFFFFFFFFu;
const P5: u32 = 0xFFFFFFFFu;
const P6: u32 = 0xFFFFFFFFu;
const P7: u32 = 0xFFFFFFFFu;

fn mul32(a: u32, b: u32) -> vec2<u32> {
    let a_lo = a & 0xFFFFu;
    let a_hi = a >> 16u;
    let b_lo = b & 0xFFFFu;
    let b_hi = b >> 16u;
    let p0 = a_lo * b_lo;
    let p1 = a_lo * b_hi;
    let p2 = a_hi * b_lo;
    let p3 = a_hi * b_hi;
    let mid = p1 + p2;
    let mid_overflow = select(0u, 1u, mid < p1);

    let lo = p0 + (mid << 16u);
    let carry_lo = select(0u, 1u, lo < p0);
    let hi = p3 + (mid >> 16u) + (mid_overflow << 16u) + carry_lo;

    return vec2<u32>(lo, hi);
}

fn fold_single(acc: ptr<function, array<u32, 17>>, h: u32, offset: u32) {
    if (h == 0u) { return; }
    let t = mul32(h, 977u);
    var sum = (*acc)[offset] + t.x;
    var carry = select(0u, 1u, sum < (*acc)[offset]);
    (*acc)[offset] = sum;
    sum = (*acc)[offset + 1u] + t.y + carry;
    carry = select(0u, 1u, sum < (*acc)[offset + 1u] || (carry == 1u && sum == (*acc)[offset + 1u]));
    (*acc)[offset + 1u] = sum;
    sum = (*acc)[offset + 1u] + h;
    carry = carry + select(0u, 1u, sum < (*acc)[offset + 1u]);
    (*acc)[offset + 1u] = sum;
    var k = offset + 2u;
    loop {
        if (carry == 0u || k >= 17u) { break; }
        sum = (*acc)[k] + carry;
        carry = select(0u, 1u, sum < (*acc)[k]);
        (*acc)[k] = sum;
        k = k + 1u;
    }
}

fn fe_cond_sub_p(val: array<u32, 8>) -> array<u32, 8> {
    var c = val;
    var tmp: array<u32, 8>;
    var borrow: u32 = 0u;
    var diff: u32;
    diff = c[0] - P0; borrow = select(0u, 1u, c[0] < P0); tmp[0] = diff;
    diff = c[1] - P1 - borrow; borrow = select(0u, 1u, c[1] < P1 + borrow || (borrow == 1u && P1 == 0xFFFFFFFFu)); tmp[1] = diff;
    diff = c[2] - P2 - borrow; borrow = select(0u, 1u, c[2] < P2 + borrow || (borrow == 1u && P2 == 0xFFFFFFFFu)); tmp[2] = diff;
    diff = c[3] - P3 - borrow; borrow = select(0u, 1u, c[3] < P3 + borrow || (borrow == 1u && P3 == 0xFFFFFFFFu)); tmp[3] = diff;
    diff = c[4] - P4 - borrow; borrow = select(0u, 1u, c[4] < P4 + borrow || (borrow == 1u && P4 == 0xFFFFFFFFu)); tmp[4] = diff;
    diff = c[5] - P5 - borrow; borrow = select(0u, 1u, c[5] < P5 + borrow || (borrow == 1u && P5 == 0xFFFFFFFFu)); tmp[5] = diff;
    diff = c[6] - P6 - borrow; borrow = select(0u, 1u, c[6] < P6 + borrow || (borrow == 1u && P6 == 0xFFFFFFFFu)); tmp[6] = diff;
    diff = c[7] - P7 - borrow; borrow = select(0u, 1u, c[7] < P7 + borrow || (borrow == 1u && P7 == 0xFFFFFFFFu)); tmp[7] = diff;
    if (borrow == 0u) { c = tmp; }
    return c;
}

fn fe_add(a: array<u32, 8>, b: array<u32, 8>) -> array<u32, 8> {
    var c: array<u32, 8>;
    var carry: u32 = 0u;
    var sum: u32;
    sum = a[0] + b[0]; carry = select(0u, 1u, sum < a[0]); c[0] = sum;
    sum = a[1] + b[1] + carry; carry = select(0u, 1u, sum < a[1] || (carry == 1u && sum == a[1])); c[1] = sum;
    sum = a[2] + b[2] + carry; carry = select(0u, 1u, sum < a[2] || (carry == 1u && sum == a[2])); c[2] = sum;
    sum = a[3] + b[3] + carry; carry = select(0u, 1u, sum < a[3] || (carry == 1u && sum == a[3])); c[3] = sum;
    sum = a[4] + b[4] + carry; carry = select(0u, 1u, sum < a[4] || (carry == 1u && sum == a[4])); c[4] = sum;
    sum = a[5] + b[5] + carry; carry = select(0u, 1u, sum < a[5] || (carry == 1u && sum == a[5])); c[5] = sum;
    sum = a[6] + b[6] + carry; carry = select(0u, 1u, sum < a[6] || (carry == 1u && sum == a[6])); c[6] = sum;
    sum = a[7] + b[7] + carry; carry = select(0u, 1u, sum < a[7] || (carry == 1u && sum == a[7])); c[7] = sum;
    if (carry == 1u) {
        var old: u32;
        old = c[0]; c[0] = c[0] + 977u; carry = select(0u, 1u, c[0] < old);
        old = c[1]; c[1] = c[1] + 1u + carry; carry = select(0u, 1u, c[1] < old || (carry == 1u && c[1] == old));
        old = c[2]; c[2] = c[2] + carry; carry = select(0u, 1u, c[2] < old);
        old = c[3]; c[3] = c[3] + carry; carry = select(0u, 1u, c[3] < old);
        old = c[4]; c[4] = c[4] + carry; carry = select(0u, 1u, c[4] < old);
        old = c[5]; c[5] = c[5] + carry; carry = select(0u, 1u, c[5] < old);
        old = c[6]; c[6] = c[6] + carry; carry = select(0u, 1u, c[6] < old);
        old = c[7]; c[7] = c[7] + carry; carry = select(0u, 1u, c[7] < old);
    }
    return fe_cond_sub_p(c);
}

fn fe_sub(a: array<u32, 8>, b: array<u32, 8>) -> array<u32, 8> {
    var c: array<u32, 8>;
    var borrow: u32 = 0u;
    var diff: u32;
    diff = a[0] - b[0]; borrow = select(0u, 1u, a[0] < b[0]); c[0] = diff;
    diff = a[1] - b[1] - borrow; borrow = select(0u, 1u, a[1] < b[1] + borrow || (borrow == 1u && b[1] == 0xFFFFFFFFu)); c[1] = diff;
    diff = a[2] - b[2] - borrow; borrow = select(0u, 1u, a[2] < b[2] + borrow || (borrow == 1u && b[2] == 0xFFFFFFFFu)); c[2] = diff;
    diff = a[3] - b[3] - borrow; borrow = select(0u, 1u, a[3] < b[3] + borrow || (borrow == 1u && b[3] == 0xFFFFFFFFu)); c[3] = diff;
    diff = a[4] - b[4] - borrow; borrow = select(0u, 1u, a[4] < b[4] + borrow || (borrow == 1u && b[4] == 0xFFFFFFFFu)); c[4] = diff;
    diff = a[5] - b[5] - borrow; borrow = select(0u, 1u, a[5] < b[5] + borrow || (borrow == 1u && b[5] == 0xFFFFFFFFu)); c[5] = diff;
    diff = a[6] - b[6] - borrow; borrow = select(0u, 1u, a[6] < b[6] + borrow || (borrow == 1u && b[6] == 0xFFFFFFFFu)); c[6] = diff;
    diff = a[7] - b[7] - borrow; borrow = select(0u, 1u, a[7] < b[7] + borrow || (borrow == 1u && b[7] == 0xFFFFFFFFu)); c[7] = diff;
    if (borrow == 1u) {
        var old: u32;
        old = c[0]; c[0] = c[0] + P0; borrow = select(0u, 1u, c[0] < old);
        old = c[1]; c[1] = c[1] + P1 + borrow; borrow = select(0u, 1u, c[1] < old || (borrow == 1u && c[1] == old));
        old = c[2]; c[2] = c[2] + P2 + borrow; borrow = select(0u, 1u, c[2] < old || (borrow == 1u && c[2] == old));
        old = c[3]; c[3] = c[3] + P3 + borrow; borrow = select(0u, 1u, c[3] < old || (borrow == 1u && c[3] == old));
        old = c[4]; c[4] = c[4] + P4 + borrow; borrow = select(0u, 1u, c[4] < old || (borrow == 1u && c[4] == old));
        old = c[5]; c[5] = c[5] + P5 + borrow; borrow = select(0u, 1u, c[5] < old || (borrow == 1u && c[5] == old));
        old = c[6]; c[6] = c[6] + P6 + borrow; borrow = select(0u, 1u, c[6] < old || (borrow == 1u && c[6] == old));
        old = c[7]; c[7] = c[7] + P7 + borrow;
    }
    return c;
}

fn fe_mul(a: array<u32, 8>, b: array<u32, 8>) -> array<u32, 8> {
    var tmp: array<u32, 16>;
    for (var i = 0u; i < 8u; i++) {
        var carry = vec2<u32>(0u, 0u);
        for (var j = 0u; j < 8u; j++) {
            let t = mul32(a[i], b[j]);
            let k = i + j;
            var acc = tmp[k] + t.x;
            var c = select(0u, 1u, acc < tmp[k]);
            tmp[k] = acc;
            acc = tmp[k + 1u] + t.y + c;
            c = select(0u, 1u, acc < tmp[k + 1u] || (c == 1u && acc == tmp[k + 1u]));
            acc = acc + carry.x;
            c = c + select(0u, 1u, acc < carry.x);
            tmp[k + 1u] = acc;
            carry = vec2<u32>(carry.y + c, 0u);
        }
        tmp[i + 8u] = tmp[i + 8u] + carry.x;
    }

    var acc: array<u32, 17>;
    for (var i = 0u; i < 16u; i++) { acc[i] = tmp[i]; }
    acc[16] = 0u;

    for (var i = 8u; i < 16u; i++) {
        let h = acc[i];
        acc[i] = 0u;
        fold_single(&acc, h, i - 8u);
        fold_single(&acc, h, i - 7u);
    }
    let h16 = acc[16];
    acc[16] = 0u;
    fold_single(&acc, h16, 8u);
    fold_single(&acc, h16, 9u);

    var res: array<u32, 8>;
    for (var i = 0u; i < 8u; i++) { res[i] = acc[i]; }
    return fe_cond_sub_p(res);
}

fn fe_square(a: array<u32, 8>) -> array<u32, 8> { return fe_mul(a, a); }
fn fe_one() -> array<u32, 8> { return array<u32, 8>(1u, 0u, 0u, 0u, 0u, 0u, 0u, 0u); }
fn fe_is_zero(a: array<u32, 8>) -> bool {
    for (var i = 0u; i < 8u; i++) { if (a[i] != 0u) { return false; } }
    return true;
}

fn fe_double(a: array<u32, 8>) -> array<u32, 8> { return fe_add(a, a); }

struct JacobianPoint {
    x: array<u32, 8>,
    y: array<u32, 8>,
    z: array<u32, 8>
}

struct AffinePoint {
    x: array<u32, 8>,
    y: array<u32, 8>
}

fn jac_add_affine(p: JacobianPoint, q: AffinePoint) -> JacobianPoint {
    if (fe_is_zero(q.x) && fe_is_zero(q.y)) {
        return p;
    }

    let u1 = p.x;
    let z1z1 = fe_square(p.z);
    let u2 = fe_mul(q.x, z1z1);
    let s1 = p.y;
    let z1z1z1 = fe_mul(z1z1, p.z);
    let s2 = fe_mul(q.y, z1z1z1);
    let h = fe_sub(u2, u1);
    let r = fe_double(fe_sub(s2, s1));

    if (fe_is_zero(h)) {
        return p;
    }

    let hh = fe_square(h);
    let i = fe_double(fe_double(hh));
    let j = fe_mul(h, i);
    let v = fe_mul(u1, i);
    let x3 = fe_sub(fe_sub(fe_square(r), j), fe_double(v));
    let y3 = fe_sub(fe_mul(r, fe_sub(v, x3)), fe_double(fe_mul(s1, j)));
    let z1_plus_1 = fe_add(p.z, fe_one());
    let z3 = fe_mul(fe_sub(fe_sub(fe_square(z1_plus_1), z1z1), fe_one()), h);

    var res: JacobianPoint;
    res.x = x3;
    res.y = y3;
    res.z = z3;
    return res;
}

struct BigInt256 {
    v0: vec4<u32>,
    v1: vec4<u32>,
}

struct Config {
    base_x: BigInt256,
    base_y: BigInt256,
    num_keys: u32,
    _pad0: u32, _pad1: u32, _pad2: u32,
    match_mode: u32,
    prefix_len: u32,
    suffix_len: u32,
    _pad3: u32,
    prefix_chars: array<vec4<u32>, 11>,
    suffix_chars: array<vec4<u32>, 11>,
}

@group(0) @binding(0) var<uniform> config: Config;
@group(0) @binding(1) var<storage, read_write> table_rw: array<AffinePoint>;
@group(0) @binding(3) var<storage, read_write> jacobian_points: array<JacobianPoint>;

fn unpack_bigint(b: BigInt256) -> array<u32, 8> {
    return array<u32, 8>(b.v0.x, b.v0.y, b.v0.z, b.v0.w, b.v1.x, b.v1.y, b.v1.z, b.v1.w);
}
