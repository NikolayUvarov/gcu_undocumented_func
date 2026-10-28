//! Explicit local check: compile as a binary and pass a separately fetched firmware path; never part of an image.
#![allow(dead_code)]
extern crate self as rtw89_wifi;
extern crate self as mind;
#[path = "../libmind/src/sha256.rs"] pub mod sha256;
#[path = "../rtw89_wifi/src/io.rs"] pub mod io;
#[path = "../rtw89_wifi/src/registers.rs"] mod registers;
#[path = "../rtw89_wifi/src/power.rs"] mod power;
#[path = "../rtw89_wifi/src/efuse.rs"] mod efuse;
#[path = "../rtw89_wifi/src/board.rs"] mod board;
#[path = "../rtw89_wifi/src/firmware.rs"] mod firmware;
#[path = "../rtw89_wifi/src/fwcmd.rs"] pub mod fwcmd;
#[path = "../rtw89_wifi/src/mac.rs"] mod mac;
#[path = "../rtw89_wifi/src/pcie.rs"] mod pcie;
#[path = "../rtw89_wifi/src/download.rs"] mod download;
#[path = "../rtw89_wifi/src/startup.rs"] mod startup;
#[path = "../rtw89_wifi/src/rx.rs"] mod rx;
#[path = "../rtw89_wifi/src/rxdesc.rs"] mod rxdesc;
#[path = "rtw89_pipeline_device.rs"] mod model;

fn roundtrip(file: &[u8], speed: u8, no_h2c: bool) {
    let mut device = model::Device::new(speed);
    let board = model::read_board(&mut device);
    let firmware = download::Verified::verify(&board, file).unwrap();
    let mut region = model::Region::new();
    let mut queue = model::prepare(&mut device, &mut region);
    device.no_h2c = no_h2c;
    let result = download::run(&mut device, &mut queue, firmware);
    if no_h2c {
        let error = result.unwrap_err();
        assert_eq!(error.stage, download::Stage::H2cReady);
        assert!(matches!(error.cause, download::Cause::Io(io::Fault { kind: io::FaultKind::Timeout, .. })));
        assert!(error.dma_stop.is_none() && error.cpu_stop.is_none());
        assert!(device.packets.is_empty());
        assert_eq!(queue.state(), fwcmd::State::Faulted);
    } else {
        assert_eq!(result.unwrap().version(), [0, 29, 29, 15]);
        assert_eq!(device.get(download::FW_CONTROL, 1) & 0xe0, 0xe0);
        assert_eq!(device.transferred, 326_472);
        assert_eq!(queue.pending(), 0); assert_eq!(queue.retained(), 8);
        assert_eq!(device.get(registers::R_AX_CMAC_FUNC_EN, 4), 0x7000803f);
        let selected = firmware::select(file, board.identity.cut, firmware::NORMAL_CE).unwrap();
        let image = firmware::parse(selected.bytes).unwrap();
        let payloads = image.sections().iter().flat_map(|section| section.bytes.chunks(2020));
        let mut actual = device.packets.iter().skip(1);
        for expected in payloads {
            let (part, bytes) = actual.next().unwrap(); assert!(*part); assert_eq!(bytes, expected);
        }
        assert!(actual.next().is_none());
    }
    assert_eq!(download::stop(&mut device, &mut queue), download::Stop { dma: None, cpu: None });
    assert_eq!(queue.state(), fwcmd::State::Faulted);
    assert_eq!(device.get(registers::R_AX_PLATFORM_ENABLE, 4) & 2, 0);
    assert_eq!(device.get(fwcmd::INIT_CONFIG, 4) & 0x2800, 0);
    assert_eq!(device.get(fwcmd::DMA_STOP, 4) & 0x001f_0f03, 0x001f_0f03);
    model::power_off(&mut device);
    assert_eq!(device.command, 0);
    println!("PASS cumulative GEN{speed}, no_h2c={no_h2c}: packets={}, modeled_us={}; modeled board/PCI, no hardware claim", device.packets.len(), device.now);
}

fn main() {
    let mut arguments = std::env::args_os().skip(1);
    let path = arguments.next().expect("usage: rtw89-pipeline-private <private rtw8852b_fw-1.bin>");
    assert!(arguments.next().is_none(), "one explicit private file path required");
    assert_eq!(std::fs::metadata(&path).unwrap().len(), download::FILE_BYTES as u64);
    let file = std::fs::read(path).unwrap();
    assert_eq!(file.len(), download::FILE_BYTES); assert_eq!(sha256::digest(&file), download::FILE_SHA256);
    roundtrip(&file, 1, false);
    roundtrip(&file, 2, false);
    roundtrip(&file, 2, true);
}
