use super::*;
use crate::phy::{self,PhyIo,RegisterPhase};

impl PhyIo for Device{
    fn pci_id(&mut self)->io::Result<u32>{Ok(0xb85210ec)}
    fn pci_command(&mut self)->io::Result<u16>{Ok(self.command)}
    fn bar_len(&self)->usize{self.regs.len()}
    fn send_rf_page(&mut self,_:u8,_:u8,_:&[u8],_:u32)->io::Result<()>{panic!("guard cannot send RF commands")}
}
struct Adapter<'a,'c,'r>{d:&'a mut Device,receiving:&'a Receiving,q:&'a mut Queue<'c>,r:&'a rx::Rings<'r>,calls:usize,time_change:i64,change_stop:bool}
impl RegisterIo for Adapter<'_,'_,'_>{
    fn read8(&mut self,at:usize)->io::Result<u8>{self.d.read8(at)}
    fn read16(&mut self,at:usize)->io::Result<u16>{self.d.read16(at)}
    fn read32(&mut self,at:usize)->io::Result<u32>{self.d.read32(at)}
    fn write8(&mut self,at:usize,v:u8)->io::Result<()>{self.d.write8(at,v)}
    fn write16(&mut self,at:usize,v:u16)->io::Result<()>{self.d.write16(at,v)}
    fn write32(&mut self,at:usize,v:u32)->io::Result<()>{self.d.write32(at,v)}
    fn now_us(&mut self)->u64{self.d.now_us()}
    fn delay_us(&mut self,us:u32)->io::Result<()>{self.d.delay_us(us)}
}
impl PhyIo for Adapter<'_,'_,'_>{
    fn pci_id(&mut self)->io::Result<u32>{Ok(0xb85210ec)}
    fn pci_command(&mut self)->io::Result<u16>{Ok(self.d.command)}
    fn bar_len(&self)->usize{self.d.regs.len()}
    fn send_rf_page(&mut self,_:u8,_:u8,_:&[u8],_:u32)->io::Result<()>{panic!("guard cannot send RF commands")}
    fn receive_permit(&mut self,us:u32)->io::Result<ReceivePermit<'_>>{
        self.calls+=1;assert_eq!(us,10_000);
        let permit=self.receiving.validate(self.d,self.q,self.r,us).map_err(|e|match e{
            Error::Register(f)=>f,_=>Fault::new(FaultKind::PowerState,0x1100,0)})?;
        self.d.now=(self.d.now as i64+self.time_change)as u64;
        if self.change_stop{self.d.put(fwcmd::DMA_STOP,4,0xb0e00);}
        Ok(permit)
    }
}
fn active(d:&mut Device,q:&mut Queue<'_>,r:&mut rx::Rings<'_>)->Receiving{
    let ready=activate_inner(d,q,r,&board(),runtime_mac::BAR_BYTES).unwrap();
    d.put(0x200,4,1<<17);d.now=100;d.writes.clear();ready
}

#[test]
fn phy_permit_allows_only_actual_owned_receive_and_command_traffic(){
    let mut d=Device::new();let mut c=Region::new(fwcmd::DMA_BYTES);let mut r=Region::new(rx::DMA_BYTES);let(mut q,mut rings)=prepared(&mut d,&mut c,&mut r);
    let receiving=active(&mut d,&mut q,&mut rings);d.put(fwcmd::DMA_BUSY,4,0x600003);
    let mut a=Adapter{d:&mut d,receiving:&receiving,q:&mut q,r:&rings,calls:0,time_change:0,change_stop:false};
    phy::guard(&mut a,&board().identity,RegisterPhase::BeforeDack).unwrap();
    a.d.put(0x8040,1,15);phy::guard(&mut a,&board().identity,RegisterPhase::AfterDack).unwrap();
    assert_eq!(a.calls,2);assert!(a.d.writes.is_empty());a.d.put(fwcmd::DMA_BUSY,4,0);stop(&mut d,&mut q,&mut rings);
}
#[test]
fn phy_default_adapter_cannot_inherit_active_rx_from_register_bits(){
    let mut d=Device::new();let mut c=Region::new(fwcmd::DMA_BYTES);let mut r=Region::new(rx::DMA_BYTES);let(mut q,mut rings)=prepared(&mut d,&mut c,&mut r);
    let _receiving=active(&mut d,&mut q,&mut rings);
    assert_eq!(phy::guard(&mut d,&board().identity,RegisterPhase::BeforeDack).unwrap_err().kind,FaultKind::Access);
    assert!(d.writes.is_empty());stop(&mut d,&mut q,&mut rings);
}
#[test]
fn phy_permit_propagates_real_ring_state_address_and_host_index_failure(){
    for (at,value) in [(0x1100,0x300000),(0x1022,255),(0x1050,1),(fwcmd::BASE_LOW,0x300000),(fwcmd::INDEX,1),(0x1a0,1)]{
        let mut d=Device::new();let mut c=Region::new(fwcmd::DMA_BYTES);let mut r=Region::new(rx::DMA_BYTES);let(mut q,mut rings)=prepared(&mut d,&mut c,&mut r);
        let receiving=active(&mut d,&mut q,&mut rings);d.put(at,4,value);
        let mut a=Adapter{d:&mut d,receiving:&receiving,q:&mut q,r:&rings,calls:0,time_change:0,change_stop:false};
        assert!(phy::guard(&mut a,&board().identity,RegisterPhase::BeforeDack).is_err());assert_eq!(a.calls,1);assert!(a.d.writes.is_empty());
        stop(&mut d,&mut q,&mut rings);
    }
}
#[test]
fn phy_permit_never_relaxes_ordinary_tx_wp_or_phase_checks(){
    for (at,value) in [(fwcmd::DMA_STOP,0xb0e00),(fwcmd::DMA_BUSY,1<<8),(fwcmd::INIT_CONFIG,0x42800),(0xc348,1),(0xc6ec,1<<7),(0x8040,15)]{
        let mut d=Device::new();let mut c=Region::new(fwcmd::DMA_BYTES);let mut r=Region::new(rx::DMA_BYTES);let(mut q,mut rings)=prepared(&mut d,&mut c,&mut r);
        let receiving=active(&mut d,&mut q,&mut rings);d.put(at,4,value);
        let mut a=Adapter{d:&mut d,receiving:&receiving,q:&mut q,r:&rings,calls:0,time_change:0,change_stop:false};
        assert!(phy::guard(&mut a,&board().identity,RegisterPhase::BeforeDack).is_err());assert!(a.d.writes.is_empty());
        d.put(fwcmd::DMA_BUSY,4,0);stop(&mut d,&mut q,&mut rings);
    }
}
#[test]
fn phy_rejects_late_or_backwards_clock_despite_actual_receive_permit(){
    for (change,kind) in [(10_000,FaultKind::Timeout),(-1,FaultKind::Clock)]{
        let mut d=Device::new();let mut c=Region::new(fwcmd::DMA_BYTES);let mut r=Region::new(rx::DMA_BYTES);let(mut q,mut rings)=prepared(&mut d,&mut c,&mut r);
        let receiving=active(&mut d,&mut q,&mut rings);
        let mut a=Adapter{d:&mut d,receiving:&receiving,q:&mut q,r:&rings,calls:0,time_change:change,change_stop:false};
        assert_eq!(phy::guard(&mut a,&board().identity,RegisterPhase::BeforeDack).unwrap_err().kind,kind);assert_eq!(a.calls,1);assert!(a.d.writes.is_empty());
        stop(&mut d,&mut q,&mut rings);
    }
}
#[test]
fn phy_rechecks_dma_stop_after_permit_observation(){
    let mut d=Device::new();let mut c=Region::new(fwcmd::DMA_BYTES);let mut r=Region::new(rx::DMA_BYTES);let(mut q,mut rings)=prepared(&mut d,&mut c,&mut r);
    let receiving=active(&mut d,&mut q,&mut rings);
    let mut a=Adapter{d:&mut d,receiving:&receiving,q:&mut q,r:&rings,calls:0,time_change:0,change_stop:true};
    assert_eq!(phy::guard(&mut a,&board().identity,RegisterPhase::BeforeDack).unwrap_err().register,fwcmd::DMA_STOP);assert!(a.d.writes.is_empty());
    stop(&mut d,&mut q,&mut rings);
}
