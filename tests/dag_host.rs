//! Host tests of libmind/src/dag.rs (issue 301-STO-0001; MC-4.2, Appendix B.3): objects as a Merkle-DAG of chunks
//! and DAG-CBOR nodes. Roots and node bytes are checked against an independent reference (the tree built by its shape
//! rule in Python with the `dag-cbor` 0.3.3 and `multiformats` 0.3.1 libraries); every non-canonical node, every node
//! or chunk out of shape and every block that does not match its CID is refused.
#![allow(dead_code)]
#[path = "../libmind/src/sha256.rs"]
mod sha256;
#[path = "../libmind/src/cid.rs"]
mod cid;
#[path = "../libmind/src/dag.rs"]
mod dag;

use cid::{Cid, Codec};
use dag::{complete, decode, encode, height, read_at, size, Blocks, Builder, Error, CHUNK, FANOUT, NODE_MAX};
use std::collections::HashMap;

/// Blocks in memory; `replace` makes the store answer a CID with other bytes.
#[derive(Default)]
struct Memory { blocks: HashMap<Cid, Vec<u8>>, puts: usize, replace: HashMap<Cid, Vec<u8>> }
impl Blocks for Memory {
    fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, Error> {
        let cid = Cid::of(codec, data);
        self.blocks.insert(cid, data.to_vec());
        self.puts += 1;
        Ok(cid)
    }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, Error> {
        let data = self.replace.get(cid).or_else(|| self.blocks.get(cid)).ok_or(Error::NotFound)?;
        out[..data.len()].copy_from_slice(data);
        Ok(data.len())
    }
    fn has(&mut self, cid: &Cid) -> Result<bool, Error> { Ok(self.blocks.contains_key(cid)) }
}

fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }
fn unhex(text: &str) -> Vec<u8> { (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect() }
fn pattern(len: usize) -> Vec<u8> { (0..len).map(|i| ((i * 31 + 7) % 251) as u8).collect() }

fn store(blocks: &mut Memory, data: &[u8], piece: usize) -> (Cid, u64) {
    let mut builder = Box::new(Builder::new());
    for part in data.chunks(piece.max(1)) { builder.write(blocks, part).unwrap(); }
    builder.finish(blocks).unwrap()
}

fn read_all(blocks: &mut Memory, root: &Cid) -> Result<Vec<u8>, Error> {
    let mut buffer = Box::new([0u8; CHUNK]);
    let total = size(blocks, root, &mut buffer)? as usize;
    let mut out = vec![0u8; total];
    let mut done = 0;
    while done < total {
        let n = read_at(blocks, root, done as u64, &mut out[done..], &mut buffer)?;
        assert!(n > 0);
        done += n;
    }
    Ok(out)
}

#[test]
fn roots_match_the_reference() {
    for (len, root, nodes) in [
        (0, "bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku", 0),
        (1, "bafkreigkgwdvr5wspzwpiutssn4xpj2i7wedshnwphhnu7ohx4pqaxxipe", 0),
        (CHUNK, "bafkreihgjfginakjmotvr5m37azopdpsn6cbkzl435sl2lvrupqyoin4ze", 0),
        (CHUNK + 1, "bafyreic3uhxyfv2fegq3ncxks5gjozd3eie5gd2mg2jx47imdt7kpoqnly", 1),
        (3 * CHUNK + 5, "bafyreihehmzwxjgnykqujlwp5t4hrhfjbizf46efitagjxwcyeriurb5qu", 1),
        (CHUNK * FANOUT, "bafyreidstuayugxp2x5xz4jfklg3t3lcc4lncadi3ygvz6tzodmhxpnszq", 1),
        (CHUNK * FANOUT + 1, "bafyreiczboab4oohlzcsoyt6wuxoz3r5m2z5blyi5pj46ah6q2pyc3d5ai", 3),
        (CHUNK * (FANOUT + 3) + 100, "bafyreihsnvgycs2hpzkpxbh5irol2xzig37dbgph5xp4bvpv7hdwvsr6ym", 3),
    ] {
        let data = pattern(len);
        let mut blocks = Memory::default();
        let (cid, total) = store(&mut blocks, &data, 5000);
        assert_eq!((cid.to_string().as_str(), total), (root, len as u64), "length {len}");
        let chunks = len.div_ceil(CHUNK).max(1);
        // Puts, not distinct blocks: the pattern repeats every 251 bytes, so chunks 251 apart are the same block.
        assert_eq!(blocks.puts, chunks + nodes, "length {len}: chunks and nodes");
        assert_eq!(read_all(&mut blocks, &cid).unwrap(), data, "length {len}");
    }
}

#[test]
fn a_node_is_the_reference_encoding() {
    let data = pattern(3 * CHUNK + 5);
    let links: Vec<Cid> = data.chunks(CHUNK).map(Cid::raw).collect();
    let mut out = [0u8; NODE_MAX];
    let len = encode(data.len() as u64, &links, &mut out);
    assert_eq!(hex(&out[..len]), "a36176016473697a6519c005656c696e6b7384d82a58250001551220e6494c86814963a758f59bf832e78df26f8415657cdf64bd2eb1a3e18721bcc9d82a58250001551220341d5fb62faacde4648d8f422188362d9e32d6fa24f6886e7fae9fe0b41fc0b7d82a58250001551220803fdf936023980816e95b029f9e90a0559e96f457484e541c3feef980f7bfffd82a58250001551220fa0ca7f18c37f43f54eb7a2cc7341a20562e3124b7c50e07f8a5a3630209612e");
    let node = decode(&out[..len]).unwrap();
    assert_eq!((node.size, node.len()), (data.len() as u64, 4));
    assert_eq!((0..4).map(|i| node.link(i)).collect::<Vec<_>>(), links);
    // A size above 2^32 takes the 8-byte form; a full node is the largest.
    let big = unhex("a36176016473697a651b0000010000000003656c696e6b7381d82a58250001551220ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert_eq!(encode((1 << 40) + 3, &[Cid::raw(b"abc")], &mut out), big.len());
    assert_eq!(out[..big.len()], big[..]);
    let full: Vec<Cid> = (0..FANOUT).map(|i| Cid::raw(&[i as u8])).collect();
    assert_eq!(encode(u64::MAX, &full, &mut out), NODE_MAX);
}

#[test]
fn the_same_bytes_give_the_same_root_however_they_are_written() {
    let data = pattern(CHUNK * 5 + 777);
    let mut blocks = Memory::default();
    let first = store(&mut blocks, &data, data.len());
    for piece in [1, 1000, CHUNK - 1, CHUNK, CHUNK + 1] { assert_eq!(store(&mut Memory::default(), &data, piece), first, "pieces of {piece}"); }
}

#[test]
fn reads_at_any_offset() {
    let data = pattern(CHUNK * FANOUT + 3 * CHUNK + 11);
    let mut blocks = Memory::default();
    let (root, _) = store(&mut blocks, &data, 65536);
    let mut buffer = Box::new([0u8; CHUNK]);
    assert_eq!(height(data.len() as u64), 2);
    for (offset, len) in [(0, 10), (CHUNK - 3, 7), (CHUNK * FANOUT - 1, 2), (data.len() - 5, 100), (12345, CHUNK * 3), (data.len(), 4)] {
        let mut out = vec![0u8; len];
        let n = read_at(&mut blocks, &root, offset as u64, &mut out, &mut buffer).unwrap();
        let end = (offset + len).min(data.len());
        assert_eq!(&out[..n], &data[offset..end], "offset {offset}");
    }
    assert_eq!(read_at(&mut blocks, &root, data.len() as u64 + 1, &mut [0u8; 1], &mut buffer), Err(Error::TooLarge));
}

#[test]
fn non_canonical_nodes_are_refused() {
    let canonical = unhex("a36176016473697a6503656c696e6b7381d82a58250001551220ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert!(decode(&canonical).is_ok());
    let link = "d82a58250001551220ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    for (case, bytes, error) in [
        ("another version", "a36176026473697a6503656c696e6b7381".to_owned() + link, Error::Version),
        ("size written longer than needed", "a36176016473697a65180365".to_owned() + "6c696e6b7381" + link, Error::Format),
        ("links before size", "a3617601656c696e6b7381".to_owned() + link + "6473697a6503", Error::Format),
        ("size before v", "a36473697a650361760165".to_owned() + "6c696e6b7381" + link, Error::Format),
        ("an extra key", "a46176016473697a6503656c696e6b7381".to_owned() + link + "6178" + "00", Error::Format),
        ("an indefinite array", "a36176016473697a6503656c696e6b739f".to_owned() + link + "ff", Error::Format),
        ("no links", "a36176016473697a6503656c696e6b7380".to_owned(), Error::Format),
        ("a link without tag 42", "a36176016473697a6503656c696e6b7381".to_owned() + &link[4..], Error::Format),
        ("a link without the identity prefix", "a36176016473697a6503656c696e6b7381d82a5824".to_owned() + &link[10..], Error::Format),
        ("a CIDv0 link", "a36176016473697a6503656c696e6b7381d82a582300".to_owned() + &link[14..], Error::Format),
        ("bytes after the node", "a36176016473697a6503656c696e6b7381".to_owned() + link + "00", Error::Format),
        ("cut short", "a36176016473697a6503656c696e6b7381".to_owned() + &link[..40], Error::Format),
    ] {
        assert_eq!(decode(&unhex(&bytes)).err(), Some(error), "{case}");
    }
    // More links than a node may have.
    let mut out = vec![0u8; NODE_MAX + 64];
    let many: Vec<Cid> = (0..FANOUT + 1).map(|i| Cid::raw(&(i as u16).to_le_bytes())).collect();
    let len = encode(1, &many, &mut out);
    assert_eq!(decode(&out[..len]).err(), Some(Error::Format));
}

// Stores a node over `size` bytes with `links` as a block and returns its CID.
fn put_node(blocks: &mut Memory, size: u64, links: &[Cid]) -> Cid {
    let mut out = [0u8; NODE_MAX];
    let len = encode(size, links, &mut out);
    blocks.put(Codec::DagCbor, &out[..len]).unwrap()
}

#[test]
fn trees_out_of_shape_are_refused() {
    let mut blocks = Memory::default();
    let a = blocks.put(Codec::Raw, &pattern(CHUNK)).unwrap();
    let b = blocks.put(Codec::Raw, &pattern(100)).unwrap();
    let mut buffer = Box::new([0u8; CHUNK]);
    let good = put_node(&mut blocks, CHUNK as u64 + 100, &[a, b]);
    assert_eq!(size(&mut blocks, &good, &mut buffer), Ok(CHUNK as u64 + 100));
    // A size that disagrees with the links, a root that should have been a chunk, a short chunk before the last, a link
    // too many, a node where a chunk belongs.
    let wrong_size = put_node(&mut blocks, CHUNK as u64 + 101, &[a, b]);
    let too_small = put_node(&mut blocks, 100, &[b]);
    let short_first = put_node(&mut blocks, CHUNK as u64 + 100, &[b, a]);
    let three = put_node(&mut blocks, CHUNK as u64 + 100, &[a, b, b]);
    let node_as_leaf = put_node(&mut blocks, CHUNK as u64 + 100, &[a, good]);
    let mut check = |root: Cid| -> Error {
        let mut out = vec![0u8; 2 * CHUNK];
        size(&mut blocks, &root, &mut buffer).and_then(|_| read_at(&mut blocks, &root, 0, &mut out, &mut buffer)).unwrap_err()
    };
    assert_eq!(check(wrong_size), Error::Shape);
    assert_eq!(check(too_small), Error::Shape);
    assert_eq!(check(short_first), Error::Shape);
    assert_eq!(check(three), Error::Shape);
    assert_eq!(check(node_as_leaf), Error::Shape);
}

#[test]
fn a_block_that_does_not_match_its_cid_is_refused() {
    let data = pattern(CHUNK * 3);
    let mut blocks = Memory::default();
    let (root, _) = store(&mut blocks, &data, CHUNK);
    let chunk = Cid::raw(&data[CHUNK..2 * CHUNK]);
    let mut forged = data[CHUNK..2 * CHUNK].to_vec();
    forged[7] ^= 1;
    blocks.replace.insert(chunk, forged);
    let mut buffer = Box::new([0u8; CHUNK]);
    let mut out = vec![0u8; 10];
    assert_eq!(read_at(&mut blocks, &root, 0, &mut out, &mut buffer), Ok(10));
    assert_eq!(read_at(&mut blocks, &root, CHUNK as u64 + 5, &mut out, &mut buffer), Err(Error::Corrupt));
    // A forged root node too.
    blocks.replace.clear();
    let mut node = blocks.blocks[&root].clone();
    let last = node.len() - 1;
    node[last] ^= 1;
    blocks.replace.insert(root, node);
    assert_eq!(size(&mut blocks, &root, &mut buffer), Err(Error::Corrupt));
}

#[test]
fn heights_follow_the_size() {
    assert_eq!([0u64, 1, CHUNK as u64].map(height), [0, 0, 0]);
    assert_eq!(height(CHUNK as u64 + 1), 1);
    assert_eq!(height((CHUNK * FANOUT) as u64), 1);
    assert_eq!(height((CHUNK * FANOUT) as u64 + 1), 2);
    assert_eq!(height(u64::MAX), 7);
}

#[test]
fn an_object_is_complete_only_with_every_block() {
    let mut buffer = Box::new([0u8; CHUNK]);
    // Three levels: 256 chunks and one more, so a node of height 2 over two of height 1.
    let data: Vec<u8> = (0..CHUNK * FANOUT + 10).map(|i| (i / CHUNK) as u8 ^ (i % 253) as u8).collect();
    let mut blocks = Memory::default();
    let (root, total) = store(&mut blocks, &data, 65536);
    assert_eq!(complete(&mut blocks, &root, &mut buffer), Ok(total));
    // A chunk missing, in the first or the last node of height 1.
    for chunk in [&data[CHUNK * 7..CHUNK * 8], &data[CHUNK * FANOUT..]] {
        let mut partial = Memory { blocks: blocks.blocks.clone(), ..Default::default() };
        partial.blocks.remove(&Cid::raw(chunk));
        assert_eq!(complete(&mut partial, &root, &mut buffer), Err(Error::NotFound));
    }
    // A node of height 1 missing.
    let first: Vec<Cid> = data[..CHUNK * FANOUT].chunks(CHUNK).map(Cid::raw).collect();
    let mut node = [0u8; NODE_MAX];
    let len = encode((CHUNK * FANOUT) as u64, &first, &mut node);
    let mut partial = Memory { blocks: blocks.blocks.clone(), ..Default::default() };
    assert!(partial.blocks.remove(&Cid::of(Codec::DagCbor, &node[..len])).is_some());
    assert_eq!(complete(&mut partial, &root, &mut buffer), Err(Error::NotFound));
    // A single chunk.
    let small = blocks.put(Codec::Raw, b"small").unwrap();
    assert_eq!(complete(&mut blocks, &small, &mut buffer), Ok(5));
    assert_eq!(complete(&mut Memory::default(), &small, &mut buffer), Err(Error::NotFound));
}
