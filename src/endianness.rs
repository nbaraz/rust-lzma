
macro_rules! impl_endian {
    ($orig:ident, $new:ident, $from:ident, $to:ident) => {
        #[derive(Clone, Copy)]
        #[repr(C)]
        pub(crate) struct $new($orig);

        impl $new {
            pub(crate) fn get(&self) -> $orig {
                $orig::$from(self.0)
            }

            pub(crate) fn new(val: $orig) -> $new {
                $new(val.$to())
            }

            pub(crate) fn set(&mut self, val: $orig) {
                self.0 = val.$to();
            }
        }
    };
}

impl_endian!(u16, u16be, from_be, to_be);
impl_endian!(u16, u16le, from_le, to_le);
impl_endian!(u32, u32be, from_be, to_be);
impl_endian!(u32, u32le, from_le, to_le);
impl_endian!(u64, u64be, from_be, to_be);
impl_endian!(u64, u64le, from_le, to_le);
