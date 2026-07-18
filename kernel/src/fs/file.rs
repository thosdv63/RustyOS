use alloc::boxed::Box;
use crate::fs::vfs::INode;

#[derive(Clone, Copy, PartialEq)]
pub enum SeekFrom {
    Start(usize),
    Current(isize),
    End(isize),
}

pub struct File {
    pub node: Box<dyn INode>,
    pub offset: usize,
    pub readable: bool,
    pub writable: bool,
}

impl File {
    pub fn new(node: Box<dyn INode>, readable: bool, writable: bool) -> Self {
        Self {
            node,
            offset: 0,
            readable,
            writable,
        }
    }

    // Reads from file and increases offset
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        if !self.readable {
            return Err("You do not have permission to read the file");
        }
        
        let bytes_read = self.node.read(self.offset, buf)?;
        self.offset += bytes_read;
        Ok(bytes_read)
    }

    // Writes file and increases offset
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, &'static str> {
        if !self.writable {
            return Err("You do not have permission to write the file.");
        }

        let bytes_written = self.node.write(self.offset, buf)?;
        self.offset += bytes_written;
        Ok(bytes_written)
    }

    pub fn seek(&mut self, pos: SeekFrom) -> Result<usize, &'static str> {
        let metadata = self.node.metadata()?;
        let file_size = metadata.size as usize;

        let new_offset = match pos {
            SeekFrom::Start(n) => n as isize,
            SeekFrom::Current(n) => self.offset as isize + n,
            SeekFrom::End(n) => file_size as isize + n,
        };

        if new_offset < 0 || new_offset > file_size as isize {
            return Err("Invalid seek offset value");
        }

        self.offset = new_offset as usize;
        Ok(self.offset)
    }
}