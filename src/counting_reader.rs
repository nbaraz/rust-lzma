use std::io::{Read, Result};

#[derive(Debug)]
pub(crate) struct CountingReader<R: Read> {
    reader: R,
    count: usize,
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let count = self.reader.read(buf)?;
        self.count += count;
        Ok(count)
    }
}

impl<R: Read> CountingReader<R> {
    pub fn new(reader: R) -> CountingReader<R> {
        CountingReader { reader, count: 0 }
    }

    pub fn count(&self) -> usize {
        self.count
    }
}
