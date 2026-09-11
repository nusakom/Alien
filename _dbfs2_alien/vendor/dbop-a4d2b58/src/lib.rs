#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use jammdb::{Bucket};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct RenameKeyOperate {
    pub old_key: String,
    pub new_key: String,
}

impl RenameKeyOperate{
    pub fn new<T:ToString>(old_key:T,new_key:T)->Self{
        Self{
            old_key:old_key.to_string(),
            new_key:new_key.to_string()
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AddKeyOperate {
    pub map: BTreeMap<String, Vec<u8>>,
}

impl AddKeyOperate {
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
        }
    }
    pub fn add_key<T:ToString>(mut self, key: T, value: Vec<u8>) ->Self {
        self.map.insert(key.to_string(), value);
        self
    }
}

/// In BucketOperate, we can do other operate continuously
///
/// For example:
/// AddBucket->AddKey>Read...
#[derive(Serialize, Deserialize, Debug)]
pub struct AddBucketOperate {
    pub key:String,
    pub other:Option<Box<OperateSet>>
}

impl AddBucketOperate {
    pub fn new<T:ToString>(key: T,other:Option<Box<OperateSet>>) -> Self {
        Self {
            key:key.to_string(),
            other,
        }
    }
    pub fn add_other(&mut self,other:Box<OperateSet>){
        self.other = Some(other);
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StepIntoOperate {
    pub key:String,
    pub other:Option<Box<OperateSet>>
}

impl StepIntoOperate{
    pub fn new<T:ToString>(key:T,other:Option<Box<OperateSet>>)->Self{
        Self{
            key:key.to_string(),
            other
        }
    }
    pub fn add_other(&mut self,other:Box<OperateSet>){
        self.other = Some(other);
    }
}


#[derive(Serialize, Deserialize, Debug)]
pub struct DeleteKeyOperate {
    pub keys: Vec<String>,
}

impl DeleteKeyOperate {
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
        }
    }
    pub fn add_key<T:ToString>(mut self, key: T)->Self{
        self.keys.push(key.to_string());
        self
    }
}


#[derive(Serialize, Deserialize, Debug)]
pub struct ReadOperate {
    pub keys: Vec<String>,
    pub buf_addr: usize,
    pub buf_size: usize,
}

impl ReadOperate {
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            buf_addr: 0,
            buf_size: 0,
        }
    }
    pub fn add_key<T:ToString>(mut self, key: T)->Self {
        self.keys.push(key.to_string());
        self
    }
    pub fn set_buf(mut self, buf_addr: usize, buf_size: usize)->Self {
        self.buf_addr = buf_addr;
        self.buf_size = buf_size;
        self
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Operate {
    RenameKey(RenameKeyOperate),
    AddKey(AddKeyOperate),
    AddBucket(AddBucketOperate),
    DeleteKey(DeleteKeyOperate),
    Read(ReadOperate),
    StepInto(StepIntoOperate),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OperateSet {
    pub operate: Vec<Operate>,
}

impl OperateSet {
    pub fn new() -> Self {
        Self {
            operate: Vec::new(),
        }
    }
    pub fn add_operate(mut self, operate: Operate)->Self {
        self.operate.push(operate);
        self
    }
}

#[macro_export]
macro_rules! add_key {
    ($ident:ident,$(($key:expr,$value:expr)),+) => {
        let mut $ident = AddKeyOperate::new();
        $(
            $ident = $ident.add_key($key,$value);
        )+
    };
}

// #[macro_export]
// macro_rules! read_key {
//     ($ident:ident,$(($key:expr,$buf_addr:expr,$buf_size:expr)),+) => {
//         let mut $ident = ReadOperate::new();
//         $(
//             $ident = $ident.add_key($key).set_buf($buf_addr,$buf_size);
//         )+
//     };
// }

#[macro_export]
macro_rules! read_key {
    ($ident:ident,[$($key:expr),+],$buf:expr,$len:expr) => {
        let mut $ident = ReadOperate::new();
        $(
            $ident = $ident.add_key($key);
        )+
        $ident = $ident.set_buf($buf as usize,$len);
    };
}

#[macro_export]
macro_rules! make_operate_set {
    ($ident:ident,[ $($operate:expr),+ ]) => {
        let mut $ident = OperateSet::new();
        $(
            $ident = $ident.add_operate($operate);
        )+
    };
}
pub enum Para<'a, 'tx> {
    Data(&'a [u8]),
    Bucket(Bucket<'a, 'tx>),
}

#[repr(C)]
pub struct MyPara<'a, 'tx>(pub Para<'a, 'tx>);
