// TODO: Remove
#![allow(unused)]

extern crate byteorder;
#[macro_use]
extern crate lazy_static;

use std::mem;
use std::io::{self, Read};
use std::slice;

use byteorder::ByteOrder;

mod varint;
mod crc;
mod endianness;
use endianness::*;


unsafe trait TransmuteSafe: Sized + Copy {
    fn from_bytes(bytes: &mut [u8]) -> Self {
        let mut temp: Self = unsafe { mem::uninitialized() };
        if bytes.len() < mem::size_of::<Self>() {
            panic!("untransmutable");
        }

        unsafe {
            let ptr: *mut Self = mem::transmute(bytes.as_ptr());
            let out_ptr = &mut temp as *mut Self;
            *out_ptr = *ptr;
        }
        temp
    }

    fn from_reader<R: Read>(reader: &mut R) -> io::Result<Self>
        where Self: Sized
    {
        let mut tmp: Self = unsafe { mem::uninitialized() };
        let arr = unsafe {
            slice::from_raw_parts_mut(&mut tmp as *mut Self as *mut u8, mem::size_of::<Self>())
        };
        reader.read_exact(arr)?;
        Ok(TransmuteSafe::from_bytes(arr))
    }
}


#[repr(C)]
struct XZStreamHeader {
    header_magic: [u8; 6],
    flags: u16be,
    crc: u32le,
}

enum XZError {
    InvalidHeaderMagic,
    InvalidFlags,
    UnsupportedFlag,
    BadCRC,
    IO(io::Error),
    Varint,
}

impl From<io::Error> for XZError {
    fn from(err: io::Error) -> XZError {
        XZError::IO(err)
    }
}

type XZResult<T> = Result<T, XZError>;


impl XZStreamHeader {
    fn verify(&self) -> Result<(), XZError> {
        if self.header_magic != [0xFD, b'7', b'z', b'X', b'Z', 0x00] {
            Err(XZError::InvalidHeaderMagic)
        } else if (self.flags.get() >> 8) != 0 {
            Err(XZError::InvalidFlags)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum CheckType {
    None = 0x00,
    CRC32 = 0x01,
    CRC64 = 0x04,
    SHA256 = 0x0A,
}

#[derive(Debug, Clone, Copy, )]
#[repr(packed)]
struct XZBlockFlags(u8);

impl XZBlockFlags {
    fn num_filters(&self) -> u8 {
        self.0 & 0x03
    }

    fn has_compressed_size(&self) -> bool {
        ((self.0 & 0x40) >> 6) != 0
    }

    fn has_uncompressed_size(&self) -> bool {
        ((self.0 & 0x80) >> 7) != 0
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(packed)]
struct XZBlockHeaderSized {
    header_size: HeaderSize,
    flags: XZBlockFlags,
}

unsafe impl TransmuteSafe for XZBlockHeaderSized {}

struct XZBlockHeader {
    sized: XZBlockHeaderSized,
    csized: Option<u64>,
    usized: Option<u64>,
    filter_flags: Vec<FilterFlags>,
    crc: u32,
}

#[derive(Debug, Clone, Copy)]
struct HeaderSize(u8);

impl HeaderSize {
    fn new(v: u8) -> Option<HeaderSize> {
        if v == 0 { None } else { Some(HeaderSize(v)) }
    }

    fn verify(self) -> bool {
        self.0 != 0
    }

    fn get(self) -> usize {
        (self.0 as usize + 1) * 4
    }
}

struct FilterFlags {
    id: u64,
    propsize: u64,
    props: Vec<u8>,
}

fn parse_block_header<R: Read>(reader: &mut R) -> XZResult<XZBlockHeader> {
    let bhs: XZBlockHeaderSized = TransmuteSafe::from_reader(reader).unwrap();

    if !bhs.header_size.verify() {
        panic!("invalid header size");
    }

    let mut buf = [0u8; 1024];
    reader.read_exact(&mut buf[..])?;
    let rest: &mut &[u8] = &mut &buf[0..bhs.header_size.get()];

    let cs = if bhs.flags.has_compressed_size() {
        Some(varint::from_reader(rest)?)
    } else {
        None
    };

    let us = if bhs.flags.has_uncompressed_size() {
        Some(varint::from_reader(rest)?)
    } else {
        None
    };

    let mut fflags = Vec::new();
    for _ in 0..bhs.flags.num_filters() {
        let id = varint::from_reader(rest)?;
        let propsize = varint::from_reader(rest)?;
        let ff = FilterFlags {
            id: id,
            propsize: propsize,
            props: rest.take(propsize).bytes().collect::<Result<_, _>>()?,
        };

        fflags.push(ff);
    }
    let rest_len = rest.len() as u64;

    if rest_len < 4 {
        panic!("wrong block header size");
    } else if rest_len != 4 {
        for b in rest.take(rest_len - 4).bytes() {
            if b? != 0x00 {
                panic!("bad padding");
            }
        }
    }

    let crc = byteorder::LittleEndian::read_u32(rest);

    Ok(XZBlockHeader {
        sized: bhs,
        csized: cs,
        usized: us,
        filter_flags: fflags,
        crc: crc,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(::u32be::new(4).0, u32::to_be(4));
    }
}
