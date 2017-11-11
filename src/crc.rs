const POLY32: u32 = 0xEDB88320;
const POLY64: u64 = 0xC96C5795D7870F42;

lazy_static! {
    static ref CRC_TABLES: ([u32; 256], [u64; 256]) = {
        let mut tables = ([0; 256], [0; 256]);

        for i in 0..256usize {
            let mut crc32: u32 = i as u32;
            let mut crc64: u64 = i as u64;

            for j in 0..8 {
                crc32 = if (crc32 & 1) != 0 {
                     (crc32 >> 1) ^ POLY32
                } else {
                    crc32 >> 1
                };

                crc64 = if (crc64 & 1) != 0 {
                     (crc64 >> 1) ^ POLY64
                } else {
                    crc64 >> 1
                };
            }

            tables.0[i] = crc32;
            tables.1[i] = crc64;
        }
        tables
    };

    static ref CRC32_TABLE: [u32; 256] = CRC_TABLES.0;
    static ref CRC64_TABLE: [u64; 256] = CRC_TABLES.1;
}


pub(crate) fn crc32<'a, I: IntoIterator<Item=&'a u8>>(iter: I, init: u32) -> u32 {
    let mut crc = !init;
    for &b in iter {
        let idx = b ^ (crc & 0xFF) as u8;
        crc = CRC32_TABLE[idx as usize] ^ (crc >> 8)
    }
    !crc
}

pub(crate) fn crc64<'a, I: IntoIterator<Item=&'a u8>>(iter: I, init: u64) -> u64 {
    let mut crc = !init;
    for &b in iter {
        let idx = b ^ (crc & 0xFF) as u8;
        crc = CRC64_TABLE[idx as usize] ^ (crc >> 8)
    }
    !crc
}
