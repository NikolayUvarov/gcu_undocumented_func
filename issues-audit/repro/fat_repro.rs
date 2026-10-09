#![allow(dead_code)]
extern crate alloc;
#[path = "../../vfs_server/src/fat.rs"]
mod fat;
use fat::{Sectors, Volume, SECTOR, Error};

struct Memory { data: Vec<u8>, fail_flush: bool }
impl Sectors for Memory {
    fn read(&mut self, lba:u32, out:&mut [u8;SECTOR])->bool {
        match self.data.get(lba as usize*SECTOR..(lba as usize+1)*SECTOR) {
            Some(s)=> {out.copy_from_slice(s);true}, None=>false
        }
    }
    fn write(&mut self, lba:u32, data:&[u8;SECTOR])->bool {
        match self.data.get_mut(lba as usize*SECTOR..(lba as usize+1)*SECTOR) {
            Some(s)=> {s.copy_from_slice(data);true}, None=>false
        }
    }
    fn flush(&mut self)->bool { !self.fail_flush }
    fn sectors(&self)->u64 { (self.data.len()/SECTOR) as u64 }
    fn writable(&self)->bool {true}
}
fn volume(sectors:usize)->Volume<Memory> {
    let mut disk=Memory{data:vec![0;sectors*SECTOR],fail_flush:false};
    fat::format(&mut disk,"AUDIT",0).unwrap();
    Volume::mount(disk).ok().unwrap()
}

#[test]
fn failed_growth_leaks_all_free_clusters() {
    let mut v=volume(128); let root=v.root();
    let mut node=v.create(&root,"file.bin",false,0).unwrap();
    v.flush().unwrap();
    let free=v.free_clusters().unwrap();
    let data=vec![0x41;(free as usize+1)*v.cluster_bytes() as usize];
    assert_eq!(v.write(&mut node,0,&data,0),Err(Error::NoSpace));
    let report=v.check().unwrap();
    println!("failed growth: free before={free}, free after={}, lost={}, directory cluster={}",v.free_clusters().unwrap(),report.lost,v.find(&root,"file.bin").unwrap().node.cluster);
    assert_eq!(report.lost,free);
    v.remove(&root,"file.bin").unwrap();
    assert_eq!(v.free_clusters().unwrap(),0);
}

#[test]
fn failed_case_change_deletes_the_original_file() {
    let mut v=volume(2048); let root=v.root();
    let mut original=v.create(&root,"alpha.txt",false,0).unwrap();
    v.write(&mut original,0,b"must survive",0).unwrap();
    let mut count=0;
    loop {
        match v.create(&root,&format!("f{count:07}"),false,0) {
            Ok(_)=>count+=1, Err(Error::NoSpace)=>break, Err(e)=>panic!("{e:?}")
        }
    }
    v.flush().unwrap();
    assert_eq!(v.rename(&root,"alpha.txt",&root,"AlPhA.txt"),Err(Error::NoSpace));
    assert_eq!(v.find(&root,"alpha.txt").unwrap_err(),Error::NotFound);
    let report=v.check().unwrap();
    println!("failed case rename: filler files={count}, original missing, lost={}",report.lost);
    assert_eq!(report.lost,1);
}

#[test]
fn allocation_clears_dirty_flag_without_a_flush() {
    let mut v=volume(16384); let root=v.root();
    assert_eq!(v.bits(),16);
    let mut node=v.create(&root,"file.bin",false,0).unwrap();
    v.flush().unwrap();
    v.write(&mut node,0,b"unflushed data",0).unwrap();
    let clean_bit = v.fat(1).unwrap() & 0x8000;
    let mut v=Volume::mount(v.disk).ok().unwrap();
    let report=v.check().unwrap();
    println!("after unflushed allocation and remount: clean bit={clean_bit:#x}, dirty={}",report.dirty);
    assert!(!report.dirty);
}

#[test]
fn failed_flush_still_marks_volume_clean() {
    let mut v=volume(16384); let root=v.root();
    v.create(&root,"file.bin",false,0).unwrap();
    assert!(v.check().unwrap().dirty);
    v.disk.fail_flush=true;
    assert_eq!(v.flush(),Err(Error::Io));
    let report=v.check().unwrap();
    println!("after failed flush: dirty={}",report.dirty);
    assert!(!report.dirty);
}
