//! Host tests of libmind/src/partition.rs (307-KRN-0177): MBR and GPT tables read from sample disks, GPT's CRCs checked,
//! partitions outside the disk left out; the store's partition known by its type and superblock both, or by `store.txt`'s
//! PARTUUID even when blank; `store.txt`'s line in both forms; the partitions vfs_server mounts (307-KRN-0229).
#[path = "../libmind/src/partition.rs"]
mod partition;
use partition::*;

// The test's own CRC-32, table-driven, checked against the standard's check value before it builds any GPT.
fn crc(data: &[u8]) -> u32 {
    let table: Vec<u32> = (0..256u32).map(|n| (0..8).fold(n, |c, _| if c & 1 != 0 { 0xEDB8_8320 ^ c >> 1 } else { c >> 1 })).collect();
    !data.iter().fold(!0u32, |c, &b| table[((c ^ b as u32) & 0xFF) as usize] ^ c >> 8)
}

struct Disk(Vec<[u8; SECTOR]>);
impl Disk {
    fn new(sectors: usize) -> Self { Disk(vec![[0; SECTOR]; sectors]) }
    fn table(&self) -> Table { let mut read = |n: u64, out: &mut [u8; SECTOR]| self.0.get(n as usize).map(|s| *out = *s).is_some(); read_table(self.0.len() as u64, &mut read) }
    fn mbr_entry(&mut self, slot: usize, kind: u8, first: u32, count: u32) {
        let e = 446 + slot * 16;
        self.0[0][e + 4] = kind;
        self.0[0][e + 8..e + 12].copy_from_slice(&first.to_le_bytes());
        self.0[0][e + 12..e + 16].copy_from_slice(&count.to_le_bytes());
        self.0[0][510] = 0x55;
        self.0[0][511] = 0xAA;
    }
}

const SIGNATURE: u32 = 0x1234_ABCD;

// The stick's layout (make_usb_image.py): two FAT partitions, then the store's, its superblock first.
fn stick(superblock: bool) -> Disk {
    let mut disk = Disk::new(8192);
    disk.0[0][440..444].copy_from_slice(&SIGNATURE.to_le_bytes());
    disk.mbr_entry(0, 0x0E, 2048, 2048);
    disk.mbr_entry(1, 0x0C, 4096, 2048);
    disk.mbr_entry(2, MBR_STORE, 6144, 2048);
    if superblock { disk.0[6144][..8].copy_from_slice(STORE_MAGIC); }
    disk
}

fn first_sector(disk: &Disk, p: &Partition) -> [u8; SECTOR] { disk.0[p.first as usize] }

const EFI_SYSTEM: &str = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b";
const UNIQUE: &str = "0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0";

fn guid(text: &str) -> [u8; 16] { match parse_id(text.as_bytes()) { Some(Id::Gpt(g)) => g, other => panic!("{text}: {other:?}") } }

// A GPT disk of `sectors` with `entries` (type, unique GUID, first, last, name) in an array of 128 entries from sector 2.
fn gpt(sectors: usize, entries: &[([u8; 16], [u8; 16], u64, u64, &str)]) -> Disk {
    let mut disk = Disk::new(sectors);
    disk.mbr_entry(0, 0xEE, 1, (sectors - 1) as u32);
    let mut array = vec![0u8; 128 * 128];
    for (i, (kind, unique, first, last, name)) in entries.iter().enumerate() {
        let e = &mut array[i * 128..(i + 1) * 128];
        e[..16].copy_from_slice(kind);
        e[16..32].copy_from_slice(unique);
        e[32..40].copy_from_slice(&first.to_le_bytes());
        e[40..48].copy_from_slice(&last.to_le_bytes());
        for (j, c) in name.encode_utf16().enumerate() { e[56 + j * 2..58 + j * 2].copy_from_slice(&c.to_le_bytes()); }
    }
    for (k, chunk) in array.chunks(SECTOR).enumerate() { disk.0[2 + k].copy_from_slice(chunk); }
    let h = &mut disk.0[1];
    h[..8].copy_from_slice(b"EFI PART");
    h[8..12].copy_from_slice(&0x0001_0000u32.to_le_bytes());
    h[12..16].copy_from_slice(&92u32.to_le_bytes());
    h[24..32].copy_from_slice(&1u64.to_le_bytes());
    h[32..40].copy_from_slice(&(sectors as u64 - 1).to_le_bytes());
    h[40..48].copy_from_slice(&34u64.to_le_bytes());
    h[48..56].copy_from_slice(&(sectors as u64 - 34).to_le_bytes());
    h[72..80].copy_from_slice(&2u64.to_le_bytes());
    h[80..84].copy_from_slice(&128u32.to_le_bytes());
    h[84..88].copy_from_slice(&128u32.to_le_bytes());
    h[88..92].copy_from_slice(&crc(&array).to_le_bytes());
    let header_crc = crc(&h[..92]);
    h[16..20].copy_from_slice(&header_crc.to_le_bytes());
    disk
}

#[test]
fn the_tests_crc_is_the_standards() {
    assert_eq!(crc(b"123456789"), 0xCBF4_3926);
}

#[test]
fn the_sticks_mbr_lists_its_three_partitions_and_the_stores_is_found() {
    let disk = stick(true);
    let table = disk.table();
    assert_eq!((table.scheme, table.problem, table.left_out), (Scheme::Mbr, None, 0));
    let parts: Vec<_> = table.partitions().copied().collect();
    assert_eq!(parts.iter().map(|p| (p.number, p.kind, p.first, p.count)).collect::<Vec<_>>(),
        [(1, Kind::Mbr(0x0E), 2048, 2048), (2, Kind::Mbr(0x0C), 4096, 2048), (3, Kind::Mbr(MBR_STORE), 6144, 2048)]);
    assert_eq!(parts[2].id, Id::Mbr { signature: SIGNATURE, number: 3 });
    assert_eq!(parts[2].label(), "");
    let found: Vec<_> = parts.iter().filter_map(|p| candidate(p, &first_sector(&disk, p), None).map(|w| (p.number, w))).collect();
    assert_eq!(found, [(3, Why::Found)], "the store's type and superblock both; the FAT partitions are not candidates");
}

#[test]
fn vfs_servers_partitions_are_the_fat_types_and_none_overlaps_the_stores() {
    // 307-KRN-0229: the stick's FAT16 and FAT32 partitions are vfs_server's, the store's and a Linux one are not.
    let mut disk = stick(true);
    disk.mbr_entry(3, 0x83, 7168, 512);
    let parts: Vec<_> = disk.table().partitions().copied().collect();
    assert_eq!(parts.iter().map(|p| p.fat_type()).collect::<Vec<_>>(), [true, true, false, false]);
    let with = |kind| { let mut p = parts[0]; p.kind = kind; p };
    assert!(MBR_FAT.iter().all(|&kind| with(Kind::Mbr(kind)).fat_type()));
    assert!(!with(Kind::Gpt([0; 16])).fat_type(), "GPT types are not read as FAT");
    // Sectors 4096 to 6143 against the store's 6144 to 8191: they touch, they do not overlap.
    assert!(!parts[1].overlaps(parts[2].first, parts[2].count) && !parts[2].overlaps(parts[1].first, parts[1].count));
    assert!(parts[1].overlaps(6143, 1) && parts[1].overlaps(0, 4097) && !parts[1].overlaps(0, 4096) && parts[1].overlaps(5000, u64::MAX));
}

#[test]
fn the_stores_type_without_its_superblock_is_not_taken_unless_named() {
    let disk = stick(false);
    let table = disk.table();
    let store = table.partitions().find(|p| p.number == 3).copied().unwrap();
    assert!(store.store_type());
    assert_eq!(candidate(&store, &first_sector(&disk, &store), None), None, "blank, of the store's type: never taken for it");
    // The owner's word takes it blank, and a superblock alone is not enough on another type.
    let named = Some(Id::Mbr { signature: SIGNATURE, number: 3 });
    assert_eq!(candidate(&store, &first_sector(&disk, &store), named), Some(Why::Named));
    let fat = table.partitions().find(|p| p.number == 1).copied().unwrap();
    let mut with_magic = [0u8; SECTOR];
    with_magic[..8].copy_from_slice(STORE_MAGIC);
    assert_eq!(candidate(&fat, &with_magic, None), None, "the superblock on a FAT partition's type");
    assert_eq!(candidate(&fat, &with_magic, Some(fat.id)), Some(Why::Named));
    let disk = stick(true);
    let store = disk.table().partitions().find(|p| p.number == 3).copied().unwrap();
    assert_eq!(candidate(&store, &first_sector(&disk, &store), named), Some(Why::FoundAndNamed));
}

#[test]
fn a_disk_with_no_store_partition_has_no_candidate() {
    let mut disk = Disk::new(8192);
    disk.mbr_entry(0, 0x0C, 2048, 6144);
    disk.0[2048][..8].copy_from_slice(STORE_MAGIC);
    let table = disk.table();
    assert_eq!(table.partitions().count(), 1);
    assert!(table.partitions().all(|p| candidate(p, &first_sector(&disk, p), None).is_none()));
}

#[test]
fn a_blank_disk_and_one_fat_volume_have_no_table() {
    assert_eq!(Disk::new(64).table().scheme, Scheme::None);
    let mut fat = Disk::new(64);
    fat.0[0][..3].copy_from_slice(&[0xEB, 0x3C, 0x90]);
    fat.0[0][11..13].copy_from_slice(&512u16.to_le_bytes());
    fat.0[0][13] = 4;
    fat.0[0][16] = 2;
    fat.0[0][510] = 0x55;
    fat.0[0][511] = 0xAA;
    assert_eq!(fat.table().scheme, Scheme::None, "a FAT volume over the whole disk, as vfs_server mounts it");
    // An MBR whose boot code starts with a jump is still a table.
    fat.0[0][13] = 0;
    fat.mbr_entry(0, 0x0C, 8, 56);
    assert_eq!(fat.table().scheme, Scheme::Mbr);
    let unreadable = { let mut read = |_: u64, _: &mut [u8; SECTOR]| false; read_table(64, &mut read) };
    assert_eq!((unreadable.scheme, unreadable.problem), (Scheme::None, Some("SECTOR 0 COULD NOT BE READ")));
}

#[test]
fn an_mbr_partition_outside_the_disk_is_left_out() {
    let mut disk = Disk::new(4096);
    disk.mbr_entry(0, 0x0C, 2048, 2048);
    disk.mbr_entry(1, MBR_STORE, 3000, 2000);
    disk.mbr_entry(2, 0x0C, 0, 10);
    let table = disk.table();
    assert_eq!(table.partitions().map(|p| p.number).collect::<Vec<_>>(), [1]);
    assert_eq!(table.problem, Some("A PARTITION OUTSIDE THE DISK, LEFT OUT"));
}

#[test]
fn a_gpt_disk_lists_its_partitions_with_labels_and_the_stores_is_found() {
    let unique = guid(UNIQUE);
    let mut disk = gpt(8192, &[(guid(EFI_SYSTEM), [1; 16], 2048, 4095, "EFI system partition"), (GPT_STORE, unique, 4096, 8191 - 34, "MIND STORE")]);
    disk.0[4096][..8].copy_from_slice(STORE_MAGIC);
    let table = disk.table();
    assert_eq!((table.scheme, table.problem), (Scheme::Gpt, None));
    let parts: Vec<_> = table.partitions().copied().collect();
    assert_eq!(parts.len(), 2);
    assert_eq!((parts[0].number, parts[0].label(), parts[0].first, parts[0].count), (1, "EFI system partition", 2048, 2048));
    assert_eq!((parts[1].number, parts[1].label(), parts[1].kind, parts[1].id), (2, "MIND STORE", Kind::Gpt(GPT_STORE), Id::Gpt(unique)));
    assert_eq!(candidate(&parts[1], &first_sector(&disk, &parts[1]), None), Some(Why::Found));
    assert_eq!(candidate(&parts[0], &first_sector(&disk, &parts[0]), None), None);
    // The store's type GUID as the issue writes it is the constant.
    assert_eq!(guid("27d2c2a1-7133-4d3d-a6ed-1de599cabdb7"), GPT_STORE);
}

#[test]
fn a_gpt_whose_header_or_entries_do_not_match_their_crc_gives_nothing() {
    let entries = [(GPT_STORE, [7; 16], 2048u64, 4095u64, "MIND STORE")];
    let mut disk = gpt(8192, &entries);
    disk.0[1][40] ^= 1;
    let table = disk.table();
    assert_eq!((table.scheme, table.partitions().count(), table.problem), (Scheme::Gpt, 0, Some("GPT HEADER CRC WRONG")));
    let mut disk = gpt(8192, &entries);
    disk.0[2][33] ^= 1;
    let table = disk.table();
    assert_eq!((table.partitions().count(), table.problem), (0, Some("GPT ENTRIES CRC WRONG")));
    let mut disk = gpt(8192, &entries);
    disk.0[1][..8].copy_from_slice(b"NOT PART");
    assert_eq!(disk.table().problem, Some("NO GPT HEADER BEHIND THE PROTECTIVE MBR"));
}

#[test]
fn a_gpt_partition_outside_the_disk_is_left_out_and_more_than_kept_are_counted() {
    let table = gpt(8192, &[(GPT_STORE, [7; 16], 2048, 9000, "PAST THE END"), (GPT_STORE, [8; 16], 2048, 4095, "INSIDE")]).table();
    assert_eq!(table.partitions().map(|p| p.label()).collect::<Vec<_>>(), ["INSIDE"]);
    assert_eq!(table.problem, Some("A PARTITION OUTSIDE THE DISK, LEFT OUT"));
    let many: Vec<_> = (0..KEPT as u64 + 3).map(|i| (GPT_STORE, [i as u8 + 1; 16], 64 + i * 8, 64 + i * 8 + 7, "P")).collect();
    let table = gpt(8192, &many).table();
    assert_eq!((table.partitions().count(), table.left_out), (KEPT, 3));
}

#[test]
fn store_txt_names_a_partition_in_either_form() {
    let gpt_id = guid(UNIQUE);
    assert_eq!(named(format!("partition {UNIQUE}\n").as_bytes()), Ok(Some(Id::Gpt(gpt_id))));
    assert_eq!(named(format!("# the store's partition\n\n  partition   {}  \n", UNIQUE.to_uppercase()).as_bytes()), Ok(Some(Id::Gpt(gpt_id))));
    assert_eq!(named(b"partition 1234abcd-03"), Ok(Some(Id::Mbr { signature: SIGNATURE, number: 3 })));
    assert_eq!(named(b"partition 1234ABCD-0a\r\n"), Ok(Some(Id::Mbr { signature: SIGNATURE, number: 10 })));
    assert_eq!(named(b""), Ok(None));
    assert_eq!(named(b"# nothing named\n"), Ok(None));
    assert!(named(b"disk 1234abcd-03").is_err(), "another word");
    assert!(named(b"partition").is_err(), "no PARTUUID");
    assert!(named(b"partition 1234abcd-03 extra").is_err(), "a word too many");
    for bad in ["1234abcd-00", "1234abc-03", "1234abcd-003", "1234abcg-03", "0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f", "0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0-1"] {
        assert!(named(format!("partition {bad}").as_bytes()).is_err(), "{bad}");
    }
}

#[test]
fn a_partuuid_reads_back_as_written() {
    let mut out = [0u8; ID_TEXT];
    for text in [UNIQUE, "1234abcd-03", "00000001-80"] {
        let id = parse_id(text.as_bytes()).unwrap();
        let n = id_text(&id, &mut out);
        assert_eq!(&out[..n], text.as_bytes());
    }
    let n = guid_text(&GPT_STORE, &mut out);
    assert_eq!(&out[..n], b"27d2c2a1-7133-4d3d-a6ed-1de599cabdb7");
}
