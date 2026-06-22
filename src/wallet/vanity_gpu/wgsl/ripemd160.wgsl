// RIPEMD-160 implementation in WGSL
// Input: 8 words (32 bytes) from SHA-256 output.
// Block size: 64 bytes.
// Padding: 0x80, zeros, length (256 bits = 0x0100).

fn rol(x: u32, n: u32) -> u32 {
    return (x << n) | (x >> (32u - n));
}

fn ripemd160(sha_out: array<u32, 8>) -> array<u32, 5> {
    // Convert SHA-256 digest (big endian words) into little endian words for RIPEMD input
    var x = array<u32, 16>();
    for (var i = 0u; i < 8u; i++) {
        let w = sha_out[i];
        x[i] = ((w & 0xFFu) << 24u) | ((w & 0xFF00u) << 8u) | ((w & 0xFF0000u) >> 8u) | ((w >> 24u));
    }
    x[8] = 0x00000080u; // padding start (len multiple of 4 bytes)
    x[9] = 0u; x[10] = 0u; x[11] = 0u; x[12] = 0u; x[13] = 0u;
    x[14] = 256u; // length in bits (32 bytes)
    x[15] = 0u;

    let rl = array<u32, 80>(
        0u,1u,2u,3u,4u,5u,6u,7u,8u,9u,10u,11u,12u,13u,14u,15u,
        7u,4u,13u,1u,10u,6u,15u,3u,12u,0u,9u,5u,2u,14u,11u,8u,
        3u,10u,14u,4u,9u,15u,8u,1u,2u,7u,0u,6u,13u,11u,5u,12u,
        1u,9u,11u,10u,0u,8u,12u,4u,13u,3u,7u,15u,14u,5u,6u,2u,
        4u,0u,5u,9u,7u,12u,2u,10u,14u,1u,3u,8u,11u,6u,15u,13u
    );
    let rr = array<u32, 80>(
        5u,14u,7u,0u,9u,2u,11u,4u,13u,6u,15u,8u,1u,10u,3u,12u,
        6u,11u,3u,7u,0u,13u,5u,10u,14u,15u,8u,12u,4u,9u,1u,2u,
        15u,5u,1u,3u,7u,14u,6u,9u,11u,8u,12u,2u,10u,0u,4u,13u,
        8u,6u,4u,1u,3u,11u,15u,0u,5u,12u,2u,13u,9u,7u,10u,14u,
        12u,15u,10u,4u,1u,5u,8u,7u,6u,2u,13u,14u,0u,3u,9u,11u
    );
    let sl = array<u32, 80>(
        11u,14u,15u,12u,5u,8u,7u,9u,11u,13u,14u,15u,6u,7u,9u,8u,
        7u,6u,8u,13u,11u,9u,7u,15u,7u,12u,15u,9u,11u,7u,13u,12u,
        11u,13u,6u,7u,14u,9u,13u,15u,14u,8u,13u,6u,5u,12u,7u,5u,
        11u,12u,14u,15u,14u,15u,9u,8u,9u,14u,5u,6u,8u,6u,5u,12u,
        9u,15u,5u,11u,6u,8u,13u,12u,5u,12u,13u,14u,11u,8u,5u,6u
    );
    let sr = array<u32, 80>(
        8u,9u,9u,11u,13u,15u,15u,5u,7u,7u,8u,11u,14u,14u,12u,6u,
        9u,13u,15u,7u,12u,8u,9u,11u,7u,7u,12u,7u,6u,15u,13u,11u,
        9u,7u,15u,11u,8u,6u,6u,14u,12u,13u,5u,14u,13u,13u,7u,5u,
        15u,5u,8u,11u,14u,14u,6u,14u,6u,9u,12u,9u,12u,5u,15u,8u,
        8u,5u,12u,9u,12u,5u,14u,6u,8u,13u,6u,5u,15u,13u,11u,11u
    );

    let kl = array<u32, 5>(0x00000000u, 0x5a827999u, 0x6ed9eba1u, 0x8f1bbcdcu, 0xa953fd4eu);
    let kr = array<u32, 5>(0x50a28be6u, 0x5c4dd124u, 0x6d703ef3u, 0x7a6d76e9u, 0x00000000u);

    var al = 0x67452301u; var bl = 0xefcdab89u; var cl = 0x98badcfeu; var dl = 0x10325476u; var el = 0xc3d2e1f0u;
    var ar = al; var br = bl; var cr = cl; var dr = dl; var er = el;

    for (var i = 0u; i < 80u; i++) {
        // Left path
        var f_left: u32;
        if (i < 16u) {
            f_left = bl ^ cl ^ dl;
        } else if (i < 32u) {
            f_left = (bl & cl) | ((~bl) & dl);
        } else if (i < 48u) {
            f_left = (bl | (~cl)) ^ dl;
        } else if (i < 64u) {
            f_left = (bl & dl) | (cl & (~dl));
        } else {
            f_left = bl ^ (cl | (~dl));
        }

        let t_left = rol(al + f_left + x[rl[i]] + kl[i / 16u], sl[i]) + el;
        al = el; el = dl; dl = rol(cl, 10u); cl = bl; bl = t_left;

        // Right path (mirrored order)
        var f_right: u32;
        if (i < 16u) {
            f_right = br ^ (cr | (~dr));
        } else if (i < 32u) {
            f_right = (br & dr) | (cr & (~dr));
        } else if (i < 48u) {
            f_right = (br | (~cr)) ^ dr;
        } else if (i < 64u) {
            f_right = (br & cr) | ((~br) & dr);
        } else {
            f_right = br ^ cr ^ dr;
        }

        let t_right = rol(ar + f_right + x[rr[i]] + kr[i / 16u], sr[i]) + er;
        ar = er; er = dr; dr = rol(cr, 10u); cr = br; br = t_right;
    }

    let h0 = 0xefcdab89u + cl + dr;
    let h1 = 0x98badcfeu + dl + er;
    let h2 = 0x10325476u + el + ar;
    let h3 = 0xc3d2e1f0u + al + br;
    let h4 = 0x67452301u + bl + cr;

    return array<u32, 5>(h0, h1, h2, h3, h4);
}
