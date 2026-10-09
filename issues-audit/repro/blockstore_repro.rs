#![allow(dead_code)]
#[path = "../../libmind/src/sha256.rs"] mod sha256;
#[path = "../../libmind/src/cid.rs"] mod cid;
#[path = "../../libmind/src/dag.rs"] mod dag;
#[path = "../../blockstore/src/store.rs"] mod store;
use store::{Device,Store,Entry,Head,Pin,Extent,SECTOR,BUFFER,LEASE_NS,Error};
use cid::Codec;
struct Memory { data:Vec<u8>, reads:usize, fail_read:Option<usize> }
impl Device for Memory {
    fn sectors(&self)->u64 { (self.data.len()/SECTOR) as u64 }
    fn writable(&self)->bool {true}
    fn read(&mut self,lba:u64,out:&mut[u8])->bool {
        self.reads+=1;
        if self.fail_read==Some(self.reads) {self.fail_read=None;return false}
        let at=lba as usize*SECTOR;
        out.copy_from_slice(&self.data[at..at+out.len()]);true
    }
    fn write(&mut self,lba:u64,data:&[u8])->bool {
        let at=lba as usize*SECTOR; self.data[at..at+data.len()].copy_from_slice(data);true
    }
    fn flush(&mut self)->bool {true}
}
#[test]
fn put_acknowledges_an_erased_block_after_failed_collection() {
    let disk=Memory{data:vec![0;128*SECTOR],reads:0,fail_read:None};
    let (mut index,mut heads,mut pins,mut holes)=(vec![Entry::EMPTY;16],vec![Head::EMPTY;8],vec![Pin::EMPTY;8],vec![Extent::EMPTY;64]);
    let (mut buffer,mut scratch)=(Box::new([0;BUFFER]),Box::new([0;dag::CHUNK]));
    let mut s=Store::mount(disk,&mut index,&mut heads,&mut pins,&mut holes,&mut buffer,&mut scratch,0).unwrap();
    let data=b"a block to store again";
    let cid=s.put(Codec::Raw,data).unwrap();
    s.put(Codec::Raw,b"another expired block").unwrap();
    s.set_time(LEASE_NS+1);
    s.device().reads=0; s.device().fail_read=Some(2);
    assert_eq!(s.collect(),Err(Error::Device));
    assert!(s.has(&cid));
    assert_eq!(s.put(Codec::Raw,data),Ok(cid));
    let result=s.get(&cid,&mut [0;128]);
    println!("after collect I/O failure: put returned Ok; immediate get={result:?}");
    assert_eq!(result,Err(Error::Corrupt));
}
