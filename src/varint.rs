use std::io::{self, Read};


pub(crate) struct PartialVarInt(u64, u8);

pub(crate) enum VarintResult {
    Full(u64),
    Partial(PartialVarInt),
    Error,
}

impl VarintResult {
    pub fn unwrap(self) -> u64 {
        if let VarintResult::Full(v) = self {
            v
        } else {
            panic!("Unwrapped partial/error varint");
        }
    }
}

impl PartialVarInt{

    pub(crate) fn empty() -> PartialVarInt {
        PartialVarInt(0, 0)
    }

    #[inline(always)]
    pub(crate) fn continue_parsing(self, b: u8) -> VarintResult {
        let PartialVarInt(mut res, count) = self;
        assert!(count <= 9);
        
        res += ((b & 0x7F) as u64) << (count * 7);
        
        if b & 0x80 == 0 {
            return VarintResult::Full(res);
        }

        match (count, b & 0x80) {
            (_, 0) => VarintResult::Full(res),
            (9, _) => VarintResult::Error,
            (c, _) => VarintResult::Partial(PartialVarInt(res, c + 1))
        }
    }
}

#[inline(always)]
pub(crate) fn from_read<R: Read>(reader: &mut R) -> io::Result<VarintResult> {
    let mut partial = PartialVarInt::empty();
    let mut buf = [0u8; 1];

    for b in reader.bytes() {
        partial = match partial.continue_parsing(b?) {
            VarintResult::Partial(p) => p,
            other => { return Ok(other); }
        };
    }

    Err(io::Error::from(io::ErrorKind::UnexpectedEof))
}
