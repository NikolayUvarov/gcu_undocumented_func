use crate::{fwcmd, io, mac, model, native_dma, pcie, power, rx, startup, tx};

#[test]
fn cold_native_layout_installs_packet_queues_before_mastering_without_enabling_them() {
    for speed in [1, 2] {
        let mut bus = model::Device::new(speed);
        model::read_board(&mut bus);
        let region = model::Region::new();
        let mut queues = unsafe { native_dma::Queues::new(&mut bus, region.base, native_dma::ALLOCATION_BYTES, 0x200000) }.unwrap();
        power::on_for_board_read(&mut io::TimedIo::new(&mut bus, 1_000_000)).unwrap();
        mac::prepare_download(&mut bus).unwrap();
        let pcie = pcie::prepare(&mut bus).unwrap();
        queues.command.install(&mut bus).unwrap();
        pcie.finish(&mut bus, &queues.command).unwrap();
        queues.receive.install(&mut bus).unwrap();
        queues.install_transmit(&mut bus).unwrap();
        assert_eq!(bus.command, 2);
        assert_eq!(queues.transmit.state(), tx::State::Installed);
        assert_eq!(queues.receive.state(), rx::State::Installed);
        assert_eq!(bus.get(fwcmd::DMA_STOP, 4) & 0x001f0f03, 0x001f0f03);
        assert_eq!(bus.get(fwcmd::INIT_CONFIG, 4) & 0x2800, 0);
        assert_eq!(bus.get(fwcmd::BASE_LOW, 4), 0x200000);
        assert_eq!(queues.receive.physical(), 0x200000 + native_dma::COMMAND_BYTES as u64);
        for (q, register) in [0x1110, 0x1118, 0x1120, 0x1128, 0x1150].into_iter().enumerate() {
            assert_eq!(bus.get(register, 4), (0x200000 + native_dma::TRANSMIT_OFFSET + q * tx::DESCRIPTORS * 8) as u32);
        }
        bus.command = 6;
        startup::command_dma(&mut bus).unwrap();
        unsafe { queues.command.start(&mut bus) }.unwrap();
        assert_eq!(queues.transmit.state(), tx::State::Installed);
        assert_eq!(bus.get(fwcmd::DMA_STOP, 4) & 0x001f0f03, 0x000b0f03);
        let stop = queues.stop_transmit(&mut bus);
        assert!(stop.request.is_none() && stop.idle.is_none());
        assert_eq!(queues.transmit.state(), tx::State::Stopped);
        let stop = crate::download::stop(&mut bus, &mut queues.command);
        assert!(stop.dma.is_none() && stop.cpu.is_none());
        model::power_off(&mut bus);
    }
}

#[test]
fn native_regions_leave_command_padding_and_unclaimed_tail_untouched() {
    assert_eq!(native_dma::ALLOCATION_BYTES, startup::DMA_GRANT_BYTES);
    assert_eq!(native_dma::COMMAND_BYTES, startup::COMMAND_REGION_BYTES);
    assert_eq!(native_dma::RECEIVE_BYTES, startup::RECEIVE_REGION_BYTES);
    assert_eq!(native_dma::TRANSMIT_OFFSET, startup::TRANSMIT_REGION_OFFSET);
    assert_eq!(native_dma::TRANSMIT_BYTES, startup::TRANSMIT_REGION_BYTES);
    assert!(native_dma::TRANSMIT_OFFSET + tx::DMA_BYTES <= native_dma::ALLOCATION_BYTES);
    let mut bus = model::Device::new(2);
    let region = model::Region::new();
    unsafe { region.base.write_bytes(0xa5, native_dma::ALLOCATION_BYTES); }
    let queues = unsafe { native_dma::Queues::new(&mut bus, region.base, native_dma::ALLOCATION_BYTES, 0x200000) }.unwrap();
    assert_ne!(queues.command.session(), queues.transmit.session());
    assert_eq!(bus.command, 2);
    drop(queues);
    let bytes = unsafe { std::slice::from_raw_parts(region.base, native_dma::ALLOCATION_BYTES) };
    assert!(bytes[fwcmd::DMA_BYTES..native_dma::COMMAND_BYTES].iter().all(|b| *b == 0xa5));
    assert!(bytes[native_dma::TRANSMIT_OFFSET..native_dma::TRANSMIT_OFFSET + tx::DMA_BYTES].iter().all(|b| *b == 0));
    assert!(bytes[native_dma::TRANSMIT_OFFSET + tx::DMA_BYTES..].iter().all(|b| *b == 0xa5));
    assert_eq!(u32::from_le_bytes(bytes[native_dma::COMMAND_BYTES + 4..native_dma::COMMAND_BYTES + 8].try_into().unwrap()),
        (0x200000 + native_dma::COMMAND_BYTES + rx::BUFFERS_AT) as u32);
}

#[test]
fn invalid_or_mastering_native_grant_cannot_initialize_the_allocation() {
    let mut bus = model::Device::new(2);
    let region = model::Region::new();
    unsafe { region.base.write_bytes(0xa5, native_dma::ALLOCATION_BYTES); }
    for (length, physical) in [(native_dma::ALLOCATION_BYTES - 1, 0x200000),
        (native_dma::ALLOCATION_BYTES, 0), (native_dma::ALLOCATION_BYTES, 0x200001),
        (native_dma::ALLOCATION_BYTES, (1u64 << 32) - 4096)] {
        assert!(matches!(unsafe { native_dma::Queues::new(&mut bus, region.base, length, physical) }, Err(native_dma::Error::Region)));
    }
    for command in [0, 6, 7, u16::MAX] {
        bus.command = command;
        assert!(matches!(unsafe { native_dma::Queues::new(&mut bus, region.base, native_dma::ALLOCATION_BYTES, 0x200000) }, Err(native_dma::Error::Command(_))));
    }
    let bytes = unsafe { std::slice::from_raw_parts(region.base, native_dma::ALLOCATION_BYTES) };
    assert!(bytes.iter().all(|b| *b == 0xa5));
    assert!(bus.writes.is_empty());
    bus.command = 0;
}

#[test]
fn premature_transmit_installation_cannot_run_packet_dma() {
    let mut bus = model::Device::new(2);
    let region = model::Region::new();
    let mut queues = unsafe { native_dma::Queues::new(&mut bus, region.base, native_dma::ALLOCATION_BYTES, 0x200000) }.unwrap();
    bus.command = 6;
    assert!(queues.install_transmit(&mut bus).is_err());
    assert_eq!(queues.transmit.state(), tx::State::Faulted);
    assert!(bus.writes.is_empty());
    bus.command = 0;
}
