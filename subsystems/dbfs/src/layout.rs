extern crate alloc;
use alloc::vec::Vec;
use core::convert::TryInto;


pub const MAGIC: &[u8; 8] = b"ALIENTFS";
pub const VERSION: u32 = 1;
pub const BLOCK_SIZE: usize = 512;

pub trait Serializer {
    fn serialize(&self) -> Vec<u8>;
    fn deserialize(data: &[u8]) -> Option<Self> where Self: Sized;
}

#[derive(Debug, Clone, Copy)]
pub struct Superblock {
    pub magic: [u8; 8],
    pub version: u32,
    pub root_inode: u64,
    pub inode_allocator_next: u64,
    pub log_start_block: u64,
    pub log_len_blocks: u64,
    pub data_start_block: u64,
}

impl Superblock {
    pub const SIZE: usize = 8 + 4 + 8 * 5; 

    pub fn new(root_inode: u64, log_start: u64, log_len: u64, data_start: u64) -> Self {
        Self {
            magic: *MAGIC,
            version: VERSION,
            root_inode,
            inode_allocator_next: 2, // Start allocation from 2
            log_start_block: log_start,
            log_len_blocks: log_len,
            data_start_block: data_start,
        }
    }
}

impl Serializer for Superblock {
    fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(Self::SIZE);
        data.extend_from_slice(&self.magic);
        data.extend_from_slice(&self.version.to_le_bytes());
        data.extend_from_slice(&self.root_inode.to_le_bytes());
        data.extend_from_slice(&self.inode_allocator_next.to_le_bytes());
        data.extend_from_slice(&self.log_start_block.to_le_bytes());
        data.extend_from_slice(&self.log_len_blocks.to_le_bytes());
        data.extend_from_slice(&self.data_start_block.to_le_bytes());
        data
    }

    fn deserialize(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SIZE { return None; }
        let magic: [u8; 8] = data[0..8].try_into().ok()?;
        if &magic != MAGIC { return None; }
        
        let version = u32::from_le_bytes(data[8..12].try_into().ok()?);
        if version != VERSION { return None; }

        Some(Self {
            magic,
            version,
            root_inode: u64::from_le_bytes(data[12..20].try_into().ok()?),
            inode_allocator_next: u64::from_le_bytes(data[20..28].try_into().ok()?),
            log_start_block: u64::from_le_bytes(data[28..36].try_into().ok()?),
            log_len_blocks: u64::from_le_bytes(data[36..44].try_into().ok()?),
            data_start_block: u64::from_le_bytes(data[44..52].try_into().ok()?),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum WalOp {
    Put = 1,
    Delete = 2,
    Commit = 3,
}

impl WalOp {
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(WalOp::Put),
            2 => Some(WalOp::Delete),
            3 => Some(WalOp::Commit),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WalEntry {
    pub tx_id: u64,
    pub op: WalOp,
    pub key: Vec<u8>,
    pub value: Vec<u8>, // Empty for Delete/Commit
    pub checksum: u32,
}

impl Serializer for WalEntry {
    fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.tx_id.to_le_bytes());
        data.push(self.op as u8);
        
        data.extend_from_slice(&(self.key.len() as u32).to_le_bytes());
        data.extend_from_slice(&self.key);
        
        data.extend_from_slice(&(self.value.len() as u32).to_le_bytes());
        data.extend_from_slice(&self.value);
        
        // Simple checksum: sum of bytes so far (excluding checksum field itself)
        // Ideally use CRC32, but for no_std minimal deps, simple sum is a placeholder
        let mut csum: u32 = 0;
        for b in &data { csum = csum.wrapping_add(*b as u32); }
        
        data.extend_from_slice(&csum.to_le_bytes());
        data
    }

    fn deserialize(data: &[u8]) -> Option<Self> {
        if data.len() < 8 + 1 + 4 + 4 + 4 { return None; } // Min header
        let mut offset = 0;
        
        let tx_id = u64::from_le_bytes(data[offset..offset+8].try_into().ok()?); offset += 8;
        let op = WalOp::from_u8(data[offset])?; offset += 1;
        
        let key_len = u32::from_le_bytes(data[offset..offset+4].try_into().ok()?) as usize; offset += 4;
        if offset + key_len > data.len() { return None; }
        let key = data[offset..offset+key_len].to_vec(); offset += key_len;
        
        let val_len = u32::from_le_bytes(data[offset..offset+4].try_into().ok()?) as usize; offset += 4;
        if offset + val_len > data.len() { return None; }
        let value = data[offset..offset+val_len].to_vec(); offset += val_len;
        
        if offset + 4 > data.len() { return None; }
        let stored_csum = u32::from_le_bytes(data[offset..offset+4].try_into().ok()?);
        
        // Verify checksum
        let mut csum: u32 = 0;
        for i in 0..offset { csum = csum.wrapping_add(data[i] as u32); }
        
        if csum != stored_csum { return None; }
        
        Some(Self {
            tx_id,
            op,
            key,
            value,
            checksum: stored_csum
        })
    }
}
