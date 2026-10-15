#![allow(dead_code)]
extern crate self as rtw89_wifi;
#[path = "../rtw89_wifi/src/io.rs"] pub mod io;
#[path = "../rtw89_wifi/src/registers.rs"] mod registers;
#[path = "../rtw89_wifi/src/power.rs"] mod power;
#[path = "../rtw89_wifi/src/efuse.rs"] mod efuse;
#[path = "../rtw89_wifi/src/board.rs"] mod board;
#[path = "../rtw89_wifi/src/fwcmd.rs"] pub mod fwcmd;
#[path = "../rtw89_wifi/src/mac.rs"] mod mac;
#[path = "../rtw89_wifi/src/pcie.rs"] mod pcie;
#[path = "../rtw89_wifi/src/startup.rs"] mod startup;
#[path = "rtw89_pipeline_device.rs"] mod model;

#[test]
fn cumulative_cold_board_power_mac_pcie_and_queue_state_agree_without_reseeding() {
    for speed in [1, 2] {
        let mut device = model::Device::new(speed);
        model::read_board(&mut device);
        let mut region = model::Region::new();
        let queue = model::prepare(&mut device, &mut region);
        assert_eq!(queue.state(), fwcmd::State::Running);
        assert_eq!(queue.pending(), 0);
        assert!(device.packets.is_empty());
        assert_eq!(device.get(registers::R_AX_PLATFORM_ENABLE, 4) & 3, 1);
        assert_eq!(device.get(fwcmd::INIT_CONFIG, 4) & 0x2800, 0x800);
        assert!(device.now < 1_000_000);
        // No consumer is asynchronous in this model and no packet was submitted; disable it before fixture teardown.
        device.command = 0;
        drop(queue);
    }
}

#[test]
fn unsupported_link_from_the_same_powered_board_never_reaches_mastering() {
    let mut device = model::Device::new(3);
    model::read_board(&mut device);
    let mut region = model::Region::new();
    let queue = region.queue(&mut device);
    power::on_for_board_read(&mut io::TimedIo::new(&mut device, 1_000_000)).unwrap();
    mac::prepare_download(&mut device).unwrap();
    let error = pcie::prepare(&mut device).unwrap_err();
    assert_eq!(error.stage, pcie::Stage::Refclock);
    assert_eq!(device.command, 2);
    assert!(device.packets.is_empty());
    assert_eq!(queue.state(), fwcmd::State::Allocated);
    model::power_off(&mut device);
}
