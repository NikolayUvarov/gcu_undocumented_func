use crate::{btc_session,fwcmd,io::{self,RegisterIo}};
use super::initial;
use std::collections::BTreeMap;

// Indirect SRAM and firmware-applied masks extend the same cold device; every command still traverses real DMA/RX.
pub(super) struct Device{
    pub base:initial::Device,pub memory:BTreeMap<u32,u32>,pub bar:usize,pub ignore_control:bool,pub ignore_unpause:bool,
    pub ignore_port:bool,pub fail_port:bool,pub late_control:bool,pub unrelated_firmware_fields:bool,
    pub window_writes:usize,pub station_packets:Vec<Vec<u8>>,pub restore_fail:bool,
    pub selector:u32,active:bool,pub late_restore:u8,restores:u8,
}
impl Device{
    pub fn new(base:initial::Device)->Self{Self{base,memory:BTreeMap::new(),bar:0x100000,ignore_control:false,ignore_unpause:false,
        ignore_port:false,fail_port:false,late_control:false,unrelated_firmware_fields:false,
        window_writes:0,station_packets:Vec::new(),restore_fail:false,selector:0x18888000,active:false,late_restore:0,restores:0}}
    fn firmware(&mut self,start:usize){
        let packets=self.base.inner.normal_packets[start..].to_vec();
        for p in packets{let h=(p[0]&3,p[0]>>2,p[1]);if ![(1,9,8),(1,8,4),(1,8,0),(1,6,0),(1,5,2)].contains(&h){continue;}
            if h==(1,9,8)&&!self.ignore_unpause{assert_eq!(p.len(),40);let mask=u32::from_le_bytes(p[24..28].try_into().unwrap());
                let pause=u32::from_le_bytes(p[8..12].try_into().unwrap());let old=self.base.inner.get(0x9eb0,4);
                self.base.inner.put(0x9eb0,4,(old&!mask)|(pause&mask));}
            if h==(1,8,0)&&self.unrelated_firmware_fields{
                self.memory.insert(0x18800000,0x1234);self.memory.insert(0x18840008,0x5678);
            }
            if h==(1,5,2){assert_eq!(p.len(),76);
                if !self.ignore_control{for i in 0..8{
                    let data=u32::from_le_bytes(p[12+4*i..16+4*i].try_into().unwrap());
                    let mask=u32::from_le_bytes(p[44+4*i..48+4*i].try_into().unwrap());
                    let at=0x18840000+4*i as u32;let old=self.memory.get(&at).copied().unwrap_or(0);
                    self.memory.insert(at,(old&!mask)|(data&mask));}}
                if self.late_control{self.base.inner.now+=600_000;}
            }
            self.station_packets.push(p);
        }
    }
    fn write(&mut self,at:usize,width:usize,value:u32)->io::Result<()>{
        if at==0xc04{assert_eq!(width,4);self.base.inner.now+=1;
            if self.restore_fail&&self.active&&value==0x18888000{return Err(io::Fault::new(io::FaultKind::Access,at,value));}
            self.selector=value;if value==0x18888000{self.restores+=1;if self.restores==self.late_restore{self.base.inner.now+=10_000;}}return Ok(());}
        if (0x40000..0x40020).contains(&at){assert_eq!(width,4);assert!(self.bar>=at+width);self.base.inner.now+=1;
            self.memory.insert(self.selector+(at-0x40000)as u32,value);self.window_writes+=1;return Ok(());}
        if at==0xc400{self.active=true;}
        if self.active&&at==0xc414&&self.ignore_port{return Ok(());}
        let count=self.base.inner.normal_packets.len();
        match width{1=>self.base.write8(at,value as u8)?,2=>self.base.write16(at,value as u16)?,_=>self.base.write32(at,value)?};
        self.firmware(count);
        if self.active&&at==0xc414&&self.fail_port{self.fail_port=false;return Err(io::Fault::new(io::FaultKind::Access,at,value));}
        Ok(())
    }
}
impl RegisterIo for Device{
    fn read8(&mut self,at:usize)->io::Result<u8>{self.base.read8(at)}
    fn read16(&mut self,at:usize)->io::Result<u16>{self.base.read16(at)}
    fn read32(&mut self,at:usize)->io::Result<u32>{
        if at==0xc04{self.base.inner.now+=1;return Ok(self.selector);}
        if (0x40000..0x40020).contains(&at){assert!(self.bar>=at+4);self.base.inner.now+=1;
            return Ok(self.memory.get(&(self.selector+(at-0x40000)as u32)).copied().unwrap_or(0));}
        self.base.read32(at)
    }
    fn write8(&mut self,at:usize,value:u8)->io::Result<()>{self.write(at,1,u32::from(value))}
    fn write16(&mut self,at:usize,value:u16)->io::Result<()>{self.write(at,2,u32::from(value))}
    fn write32(&mut self,at:usize,value:u32)->io::Result<()>{self.write(at,4,value)}
    fn now_us(&mut self)->u64{self.base.now_us()}
    fn delay_us(&mut self,us:u32)->io::Result<()>{self.base.delay_us(us)}
}
impl fwcmd::Bus for Device{fn pci_command(&mut self)->io::Result<u16>{self.base.pci_command()}}
impl btc_session::DeviceIo for Device{
    fn pci_id(&mut self)->io::Result<u32>{Ok(0xb85210ec)}
    fn bar_len(&self)->usize{self.bar}
}
