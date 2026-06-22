#include <metal_stdlib>
using namespace metal;

struct PatternConfig {
    uint mode;
    uint prefix_len;
    uint suffix_len;
    uint reserved;
    uint prefix_chars[44];
    uint suffix_chars[44];
};

struct ProbeConfig {
    uint batch_size;
    uint reserved0;
    uint reserved1;
    uint reserved2;
    PatternConfig pattern;
};

struct ProbeResult {
    atomic_uint marker;
    uint first_key_word;
    uint pattern_mode;
    uint batch_size;
};

struct PubkeyStageResult {
    uint marker;
    uint first_key_word;
    uint batch_size;
    uint reserved;
};

inline void add_u64_le(thread uint words[8], ulong value) {
    ulong carry = value;
    for (uint i = 0; i < 8; i++) {
        ulong sum = (ulong)words[i] + (carry & 0xfffffffful);
        words[i] = (uint)(sum & 0xfffffffful);
        carry = (carry >> 32) + (sum >> 32);
        if (carry == 0) {
            break;
        }
    }
}

kernel void btcc_vanity_probe(
    constant ProbeConfig &config [[buffer(0)]],
    constant uint *start_key [[buffer(1)]],
    device ProbeResult *result [[buffer(2)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid == 0) {
        atomic_store_explicit(&result->marker, 1u, memory_order_relaxed);
        result->first_key_word = start_key[0];
        result->pattern_mode = config.pattern.mode;
        result->batch_size = config.batch_size;
    }
}

kernel void btcc_expand_keys(
    constant ProbeConfig &config [[buffer(0)]],
    constant uint *start_key [[buffer(1)]],
    device uint *expanded_keys [[buffer(2)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= config.batch_size) {
        return;
    }

    thread uint words[8];
    for (uint i = 0; i < 8; i++) {
        words[i] = start_key[i];
    }

    add_u64_le(words, (ulong)gid);

    uint base = gid * 8u;
    for (uint i = 0; i < 8; i++) {
        expanded_keys[base + i] = words[i];
    }
}

kernel void btcc_pubkey_stage_probe(
    constant ProbeConfig &config [[buffer(0)]],
    constant uint *expanded_keys [[buffer(1)]],
    device PubkeyStageResult *result [[buffer(2)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid == 0) {
        result->marker = 1u;
        result->first_key_word = expanded_keys[0];
        result->batch_size = config.batch_size;
        result->reserved = 0u;
    }
}
