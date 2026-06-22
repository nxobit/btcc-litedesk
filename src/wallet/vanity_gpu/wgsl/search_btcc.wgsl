var<workgroup> prefix: array<array<u32, 8>, 64>;
var<workgroup> suffix: array<array<u32, 8>, 64>;
var<workgroup> inv_total_shared: array<u32, 8>;

fn pattern_char(chunks: array<vec4<u32>, 11>, index: u32) -> u32 {
    let chunk = chunks[index / 4u];
    let offset = index % 4u;
    if (offset == 0u) { return chunk.x; }
    if (offset == 1u) { return chunk.y; }
    if (offset == 2u) { return chunk.z; }
    return chunk.w;
}

fn bech32_charset(value: u32) -> u32 {
    let chars = array<u32, 32>(
        113u, 112u, 122u, 114u, 121u, 57u, 120u, 56u,
        103u, 102u, 50u, 116u, 118u, 100u, 119u, 48u,
        115u, 51u, 106u, 110u, 53u, 52u, 107u, 104u,
        99u, 101u, 54u, 109u, 117u, 97u, 55u, 108u
    );
    return chars[value];
}

fn hash160_byte(words: array<u32, 5>, index: u32) -> u32 {
    let word = words[index / 4u];
    let shift = (index % 4u) * 8u;
    return (word >> shift) & 0xffu;
}

fn bech32_polymod_step(pre: u32, value: u32) -> u32 {
    let b = pre >> 25u;
    var chk = ((pre & 0x1ffffffu) << 5u) ^ value;
    if ((b & 1u) != 0u) { chk = chk ^ 0x3b6a57b2u; }
    if ((b & 2u) != 0u) { chk = chk ^ 0x26508e6du; }
    if ((b & 4u) != 0u) { chk = chk ^ 0x1ea119fau; }
    if ((b & 8u) != 0u) { chk = chk ^ 0x3d4233ddu; }
    if ((b & 16u) != 0u) { chk = chk ^ 0x2a1462b3u; }
    return chk;
}

fn build_btcc_address_chars(hash160: array<u32, 5>) -> array<u32, 42> {
    var data = array<u32, 33>();
    data[0] = 0u;

    var acc = 0u;
    var bits = 0u;
    var data_idx = 1u;
    for (var i = 0u; i < 20u; i = i + 1u) {
        acc = (acc << 8u) | hash160_byte(hash160, i);
        bits = bits + 8u;
        loop {
            if (bits < 5u) { break; }
            bits = bits - 5u;
            data[data_idx] = (acc >> bits) & 31u;
            data_idx = data_idx + 1u;
        }
    }
    if (bits > 0u) {
        data[data_idx] = (acc << (5u - bits)) & 31u;
        data_idx = data_idx + 1u;
    }

    var checksum = array<u32, 6>();
    var chk = 1u;
    chk = bech32_polymod_step(chk, 3u);
    chk = bech32_polymod_step(chk, 3u);
    chk = bech32_polymod_step(chk, 0u);
    chk = bech32_polymod_step(chk, 3u);
    chk = bech32_polymod_step(chk, 3u);
    for (var i = 0u; i < 33u; i = i + 1u) {
        chk = bech32_polymod_step(chk, data[i]);
    }
    for (var i = 0u; i < 6u; i = i + 1u) {
        chk = bech32_polymod_step(chk, 0u);
    }
    let polymod = chk ^ 1u;
    for (var i = 0u; i < 6u; i = i + 1u) {
        checksum[i] = (polymod >> (5u * (5u - i))) & 31u;
    }

    var chars = array<u32, 42>();
    chars[0] = 99u;
    chars[1] = 99u;
    chars[2] = 49u;
    for (var i = 0u; i < 33u; i = i + 1u) {
        chars[3u + i] = bech32_charset(data[i]);
    }
    for (var i = 0u; i < 6u; i = i + 1u) {
        chars[36u + i] = bech32_charset(checksum[i]);
    }
    return chars;
}

fn match_btcc_address(chars: array<u32, 42>) -> bool {
    if (config.match_mode == 1u || config.match_mode == 3u) {
        for (var i = 0u; i < config.prefix_len; i = i + 1u) {
            if (chars[i] != pattern_char(config.prefix_chars, i)) {
                return false;
            }
        }
    }

    if (config.match_mode == 2u || config.match_mode == 3u) {
        for (var i = 0u; i < config.suffix_len; i = i + 1u) {
            let addr_idx = 42u - config.suffix_len + i;
            if (chars[addr_idx] != pattern_char(config.suffix_chars, i)) {
                return false;
            }
        }
    }

    return true;
}

@compute @workgroup_size(64)
fn batch_normalize_btcc_match(@builtin(global_invocation_id) gid: vec3<u32>,
                              @builtin(local_invocation_id) lid: vec3<u32>) {
    let local_idx = lid.x;
    let global_idx = gid.x;

    var z_val: array<u32, 8>;
    if (global_idx < config.num_keys) {
        z_val = jacobian_points[global_idx].z;
    } else {
        z_val = fe_one();
    }
    prefix[local_idx] = z_val;
    suffix[local_idx] = z_val;
    workgroupBarrier();

    for (var stride = 1u; stride < 64u; stride = stride * 2u) {
        var val = prefix[local_idx];
        if (local_idx >= stride) {
            val = fe_mul(prefix[local_idx - stride], val);
        }
        workgroupBarrier();
        prefix[local_idx] = val;
        workgroupBarrier();
    }

    for (var stride = 1u; stride < 64u; stride = stride * 2u) {
        var val = suffix[local_idx];
        if (local_idx + stride < 64u) {
            val = fe_mul(val, suffix[local_idx + stride]);
        }
        workgroupBarrier();
        suffix[local_idx] = val;
        workgroupBarrier();
    }

    if (local_idx == 0u) {
        let total = prefix[63];
        if (fe_is_zero(total)) {
            inv_total_shared = fe_one();
        } else {
            inv_total_shared = fe_inv(total);
        }
    }
    workgroupBarrier();
    let inv_total = inv_total_shared;

    var z_inv: array<u32, 8>;
    if (local_idx == 0u) {
        z_inv = fe_mul(inv_total, suffix[1]);
    } else if (local_idx == 63u) {
        z_inv = fe_mul(prefix[62], inv_total);
    } else {
        let tmp = fe_mul(prefix[local_idx - 1u], inv_total);
        z_inv = fe_mul(tmp, suffix[local_idx + 1u]);
    }

    if (global_idx >= config.num_keys) { return; }

    let z_inv2 = fe_square(z_inv);
    let z_inv3 = fe_mul(z_inv2, z_inv);

    let p = jacobian_points[global_idx];
    var p_aff: AffinePoint;
    p_aff.x = fe_mul(p.x, z_inv2);
    p_aff.y = fe_mul(p.y, z_inv3);

    let parity = p_aff.y[0] & 1u;
    let sha_out = sha256_compressed_pubkey(parity, p_aff.x);
    let ripemd_out = ripemd160(sha_out);
    let chars = build_btcc_address_chars(ripemd_out);

    if (match_btcc_address(chars)) {
        atomicMin(&match_result[0], global_idx);
    }
}
