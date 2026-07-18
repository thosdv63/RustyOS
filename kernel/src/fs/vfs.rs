use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::string::String;

#[derive(Debug, Clone, PartialEq)]
pub enum NodeType {
    File,
    Directory,
}

#[derive(Debug, Clone)]
pub struct VfsMetadata {
    pub name: String,
    pub size: u32,
    pub node_type: NodeType,
}

pub trait INode {
    fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize, &'static str>;
    fn write(&mut self, offset: usize, buf: &[u8]) -> Result<usize, &'static str>;
    fn metadata(&self) -> Result<VfsMetadata, &'static str>;
    
    fn read_dir(&self) -> Result<Vec<VfsMetadata>, &'static str> {
        Err("This node isn't a directory")
    }

    fn find(&self, _name: &str) -> Result<Box<dyn INode>, &'static str> {
        Err("This node isn't a directory")
    }
}

pub trait FileSystem {
    fn root_node(&self) -> Result<Box<dyn INode>, &'static str>;
    fn name(&self) -> &'static str;
}