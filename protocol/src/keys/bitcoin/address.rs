pub fn encode(hash: &[u8], prefix: &str, version_bit: u8) -> String {
    let mut payload: Vec<u8> = vec![version_bit];
    payload.extend_from_slice(hash);

    let mut payload = bech32::convert_bits(&payload, 8, 5, true).unwrap();
    let checksum = calculate_checksum(&prefix, &payload);
    payload.extend_from_slice(&checksum);

    format!("{}:{}", prefix, b32encode(&payload))
}

fn polymod(v: &[u8]) -> u64 {
    let mut c: u64 = 1;

    for &d in v {
        let c0: u8 = (c >> 35) as u8;
        c = ((c & 0x07ffffffff) << 5) ^ u64::from(d);

        if (c0 & 0x01) != 0 {
            c ^= 0x98f2bc8e61;
        }
        if (c0 & 0x02) != 0 {
            c ^= 0x79b76d99e2;
        }
        if (c0 & 0x04) != 0 {
            c ^= 0xf33e5fb3c4;
        }
        if (c0 & 0x08) != 0 {
            c ^= 0xae2eabe2a8;
        }
        if (c0 & 0x10) != 0 {
            c ^= 0x1e4f43e470;
        }
    }

    return c ^ 1;
    // c
}

fn prefix_expand(prefix: &str) -> Vec<u8> {
    let mut expanded_prefix: Vec<u8> = prefix.bytes().map(|x| (x & 0x1F) as u8).collect();
    expanded_prefix.push(0);

    expanded_prefix
}

fn calculate_checksum(prefix: &str, payload: &[u8]) -> Vec<u8> {
    let mut combined_data = prefix_expand(&prefix);
    combined_data.extend_from_slice(&payload);
    combined_data.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);

    let poly = polymod(&combined_data);

    let mut out = Vec::new();
    for i in 0..8 {
        // out.push(((poly >> (5 * i)) & 0x1F) as u8);
        out.push(((poly >> 5 * (7 - i)) & 0x1F) as u8);
    }

    out
}

const CHARSET: [char; 32] = [
    'q', 'p', 'z', 'r', 'y', '9', 'x', '8', //  +0
    'g', 'f', '2', 't', 'v', 'd', 'w', '0', //  +8
    's', '3', 'j', 'n', '5', '4', 'k', 'h', // +16
    'c', 'e', '6', 'm', 'u', 'a', '7', 'l', // +24
];

fn b32encode(inputs: &[u8]) -> String {
    let mut out = String::new();

    for &char_code in inputs {
        out.push(CHARSET[char_code as usize]);
    }

    out
}
