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
mod counting_reader;
use counting_reader::CountingReader;


#[derive(Debug)]
pub enum XZErrorKind {
    InvalidHeaderMagic,
    InvalidFlags,
    InvalidHeaderSize,
    UnsupportedFlag,
    BadCRC,
    BadPadding,
    IO(io::Error),
    Varint,
}

#[derive(Debug)]
pub struct XZError {
    kind: XZErrorKind,
    location: Option<usize>,
}

impl XZError {
    fn add_count<R: Read>(mut self, r: &CountingReader<R>) -> XZError {
        self.location = Some(r.count());
        self
    }
}

impl From<io::Error> for XZError {
    fn from(err: io::Error) -> XZError {
        XZError {
            kind: XZErrorKind::IO(err),
            location: None,
        }
    }
}


impl From<XZErrorKind> for XZError {
    fn from(kind: XZErrorKind) -> XZError {
        XZError {
            kind: kind,
            location: None,
        }
    }
}


type XZResult<T> = Result<T, XZError>;

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

fn transmute_from_reader<T: TransmuteSafe, R: Read>(reader: &mut R) -> io::Result<T> {
    <T as TransmuteSafe>::from_reader(reader)
}


unsafe impl<T> TransmuteSafe for T where T: Sized + Copy {}


#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct XZStreamHeader {
    header_magic: [u8; 6],
    flags: u16be,
    crc: u32le,
}


impl XZStreamHeader {
    fn from_reader<R: Read>(reader: &mut R) -> XZResult<XZStreamHeader> {
        let hdr: XZStreamHeader = TransmuteSafe::from_reader(reader)?;

        if hdr.header_magic != [0xFD, b'7', b'z', b'X', b'Z', 0x00] {
            Err(XZErrorKind::InvalidHeaderMagic.into())
        } else if (hdr.flags.get() >> 8) != 0 || (hdr.flags.get() & 0xF0) != 0 {
            // TODO: More verification
            Err(XZErrorKind::InvalidFlags.into())
        } else {
            Ok(hdr)
        }
    }

    fn check_type(&self) -> Option<CheckType> {
        use CheckType::*;

        Some(match self.flags.get() & 0x0F {
            0x00 => None,
            0x01 => CRC32,
            0x04 => CRC64,
            0x0A => SHA256,
            _ => return Option::None,
        })
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

impl CheckType {
    fn check_size(self) -> u8 {
        use CheckType::*;

        match self {
            None => 0,
            CRC32 => 4,
            CRC64 => 8,
            SHA256 => 32,
        }
    }
}

#[derive(Clone, Copy)]
#[repr(packed)]
struct XZBlockFlags(u8);

impl XZBlockFlags {
    fn is_ok(&self) -> bool {
        self.0 & 0x3C == 0
    }

    fn num_filters(&self) -> u8 {
        (self.0 & 0x03) + 1
    }

    fn has_compressed_size(&self) -> bool {
        ((self.0 & 0x40) >> 6) != 0
    }

    fn has_uncompressed_size(&self) -> bool {
        ((self.0 & 0x80) >> 7) != 0
    }
}


impl std::fmt::Debug for XZBlockFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "XZBlockFlags{{ ok: {}, num_filters: {}, has_cs: {}, has_us: {} }}",
            self.is_ok(),
            self.num_filters(),
            self.has_compressed_size(),
            self.has_uncompressed_size()
        )
    }
}

#[derive(Debug)]
struct XZBlockHeader {
    size: u16,
    flags: XZBlockFlags,
    csized: Option<u64>,
    usized: Option<u64>,
    filter_flags: Vec<FilterFlags>,
    crc: u32,
}

#[derive(Debug, Clone, Copy)]
struct HeaderSize(u8);

impl HeaderSize {
    fn disambiguate(self) -> HeaderKind {
        if self.0 == 0 {
            HeaderKind::Index
        } else {
            HeaderKind::Block((u16::from(self.0) + 1) * 4)
        }
    }
}


#[derive(Debug)]
enum HeaderKind {
    Block(u16),
    Index,
}


#[derive(Debug)]
struct FilterFlags {
    id: u64,
    propsize: u64,
    props: Vec<u8>,
}

fn parse_block_header<R: Read>(reader: &mut R, header_size: u16) -> XZResult<XZBlockHeader> {
    let flags: XZBlockFlags = transmute_from_reader(reader)?;

    if !flags.is_ok() {
        return Err(XZErrorKind::InvalidFlags.into());
    }

    let mut buf = [0u8; 1024];
    let header_data = &mut buf[0..header_size as usize - 2];
    reader.read_exact(header_data)?;
    let rest: &mut &[u8] = &mut &*header_data;

    let cs = if flags.has_compressed_size() {
        Some(varint::from_reader(rest)?)
    } else {
        None
    };

    let us = if flags.has_uncompressed_size() {
        Some(varint::from_reader(rest)?)
    } else {
        None
    };

    let mut fflags = Vec::new();
    for _ in 0..flags.num_filters() {
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
        return Err(XZErrorKind::BadPadding.into());
    } else if rest_len != 4 {
        for b in rest.take(rest_len - 4).bytes() {
            if b? != 0x00 {
                return Err(XZErrorKind::BadPadding.into());
            }
        }
    }

    let crc = byteorder::LittleEndian::read_u32(rest);

    Ok(XZBlockHeader {
        size: header_size,
        flags: flags,
        csized: cs,
        usized: us,
        filter_flags: fflags,
        crc: crc,
    })
}

fn parse_xz_block<R: Read>(reader: &mut R, header_size: u16) -> XZResult<()> {
    let block_header = parse_block_header(reader, header_size)?;
    // TODO: parse block
    Ok(())
}

macro_rules! addc {
    ($expr:expr, $reader:ident) => ($expr.map_err(|e| XZError::from(e).add_count($reader)))
}

pub fn parse_xz_stream<R: Read>(reader: &mut R) -> XZResult<()> {
    let reader = &mut CountingReader::new(reader);
    let stream_header = addc!(XZStreamHeader::from_reader(reader), reader)?;
    while let HeaderKind::Block(header_size) =
        addc!(transmute_from_reader::<HeaderSize, _>(reader), reader)?
            .disambiguate()
    {
        addc!(parse_xz_block(reader, header_size), reader)?;
    }
    // TODO: parse index
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(::u32be::new(4).0, u32::to_be(4));
    }
}
