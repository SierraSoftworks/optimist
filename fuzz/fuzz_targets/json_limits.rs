pub(crate) const MAX_INPUT_BYTES: usize = 16 * 1024;
const MAX_DEPTH: usize = 16;
const MAX_COLLECTION_ITEMS: u8 = 32;
const MAX_STRING_BYTES: usize = 512;

pub(crate) fn within_limits(data: &[u8]) -> bool {
    if data.len() > MAX_INPUT_BYTES {
        return false;
    }

    let mut collection_commas = [0_u8; MAX_DEPTH];
    let mut depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut string_bytes = 0_usize;

    for &byte in data {
        if in_string {
            string_bytes += 1;
            if escaped {
                escaped = false;
            } else {
                match byte {
                    b'\\' => escaped = true,
                    b'"' => in_string = false,
                    _ => {}
                }
            }
            if string_bytes > MAX_STRING_BYTES {
                return false;
            }
            continue;
        }

        match byte {
            b'"' => {
                in_string = true;
                string_bytes = 0;
            }
            b'{' | b'[' => {
                if depth == MAX_DEPTH {
                    return false;
                }
                collection_commas[depth] = 0;
                depth += 1;
            }
            b'}' | b']' => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            b',' if depth > 0 => {
                let commas = &mut collection_commas[depth - 1];
                *commas += 1;
                if *commas >= MAX_COLLECTION_ITEMS {
                    return false;
                }
            }
            _ => {}
        }
    }

    !in_string && depth == 0
}
