//! Bounded wlanhw 2.0 adapter for one actually admitted channel; native construction remains separate.

use crate::{band,btc_session::{DeviceIo,data_mode::{self,DataSession}},country::{Limits,Refusal},frames,receive_service::Event,rx,rxdesc,tx};
use mind::{dev,idl::{codec::List,wire,wlanhw},ipc::Endpoint,sys,time};

const RECEIVE:[usize;2]=[9,10];
const POLL_MS:u32=2;
// Receive passes a step: each takes one descriptor of each ring. One a step let the Ryzen's packet buffer fill on a busy
// channel (2026-11-08: CMAC RX DMA error, then the firmware's L0 recovery); Linux's NAPI takes 64 a pass.
// A step drains up to DRAIN packets by the receive alone, and the data mode's checks are made once in CHECK_MS
// (213-DRV-0032: made for every packet, and with 32 polls a step at a full buffer of four frames, a step took 20 to
// 33 ms; a download came at 19 KB/s on the Ryzen, and at 157 KB/s without that).
const DRAIN:usize=64;
const CHECK_MS:u64=10;
const CHECK_LATEST_MS:u64=100;
const WORK_US:u32=100_000;
const COMMAND_US:u32=100_000;
// A send times out here this much before the data mode's own deadline for it, so that a frame that never ends stops
// the service as TransmitTimeout with what it saw, not as whatever register read ran out of its time (the Ryzen, 2026-11-08).
const FRAME_MARGIN_MS:u64=10;
// DEBUG (the Ryzen, 2026-11-08: everything runs first, hardening later): a send makes at most this many submissions, which
// fit its time at about 7 ms each; wlan's 15 at 24 Mb/s, none acknowledged, outran it, so its rate never stepped down.
const SEND_TRIES:u8=8;
// DEBUG (the same): a send that outruns its time is answered as sent and not acknowledged, and its release is awaited
// LATE_MS more while nothing else is sent; one that outruns that too stops the service. False: TransmitTimeout at once.
// The hardware holds a frame while the channel is taken and while its RTS goes unanswered: on the Ryzen's crowded
// channel the releases came up to 1.4 s after the submission, most of them acknowledged (2026-11-21). The 1 s that
// stood here stopped the driver over frames on their way, and each stop cost the station its address and connections.
const KEEP_LATE:bool=true;
const LATE_MS:u64=5000;
// 213-DRV-0104 step 2: what a late send is answered at its deadline. NotAcknowledged is the driver as it was; the day
// wlanhw's `sent` has a pending outcome (NET's to version), a `Pending` answer goes here, the frame kept pending as now.
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
enum LateAnswer{NotAcknowledged}
const LATE_ANSWER:LateAnswer=LateAnswer::NotAcknowledged;
fn late_answer(attempts:u8)->wlanhw::Sent{match LATE_ANSWER{LateAnswer::NotAcknowledged=>wlanhw::Sent{acked:false,attempts}}}
// 213-DRV-0104, a bench switch: true puts the next send on the ring behind a late one, which is parked (its release
// awaited by its sequence, not resubmitted), instead of refusing every send busy until that release; one parked at a time.
const SEND_BEHIND_LATE:bool=false;
// 213-DRV-0104, a bench mode: true alternates SEND_BEHIND_LATE by the minute line's minutes, the first whole one off, then
// on, turn and turn about; a switch waits for a poll with no send pending or parked. Each minute's line says what it held.
const SEND_BEHIND_LATE_ALTERNATES:bool=false;
const WAIT_MS:u64=1000;
const WAITERS:usize=4;
const FRAMES:usize=16;
const FRAME_BYTES:usize=frames::HEADER_BYTES+frames::FRAME_MAX;
// DEBUG (the maintainer, 2026-11-08: everything runs first, hardening later): a country that refuses the admitted
// channel is logged as such, answered as applied, and the radio keeps sending. False is wlanhw 1.2's `failed`.
const KEEP_SENDING:bool=true;
// 801-DRV-0071: no Wi-Fi frame for this long while the filter lets a network's beacons in is a stalled receive, and the
// service stops so that the driver restarts (the Ryzen, 2026-11-13: CoolNet2 lost 55 s after joining, no line from it).
pub(crate) const QUIET_MS:u64=10_000;
// Unicast sends in a row not acknowledged, over at least UNHEARD_MS: logged once a run, the service goes on.
pub(crate) const UNHEARD:u32=16;
pub(crate) const UNHEARD_MS:u64=5_000;
// 213-DRV-0091 step 2: Linux's msleep(beacon_int + 1) before a port update, at the bench's 100 TU.
const PORT_WAIT_MS:u64=101;
// 213-DRV-0087: what became of the PPDU status reports is logged this often while it changes.
const SIGNALS_MS:u64=60_000;
// 213-DRV-0090: no beacon of the joined BSS for this long, once one was heard, is a hole (they come about ten a second);
// the first HOLES_PROBED of a run are logged with the registers, and the minute's line has them on its first minute
// and every PROBE_MINUTES after.
const HOLE_MS:u64=1000;
// While no beacon of the BSS reaches the host, a hole by frames: idle gaps of a second or so are no hole.
const FRAMES_HOLE_MS:u64=3000;
const HOLES_PROBED:u32=12;
const PROBE_MINUTES:u32=10;

#[derive(Debug)]
pub(crate) enum Fatal{Radio(data_mode::Failure),Ipc(sys::Error),Clock,TransmitTimeout(tx::Seen),UnknownAttempts,RetainedControl,Frame(frames::Error),
    ReceiveStalled(Stall)}
/// A stalled receive (QUIET_MS): how long no frame came, the frames before, and the sends finished and acknowledged since.
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) struct Stall{pub quiet_ms:u64,pub frames:u64,pub sent:u32,pub acked:u32}
// `grace_us`: the data mode's grace when the send began; a recovery it outlasts since moves `deadline` (OUTLAST_SER).
// `call` is None once a late send was answered (KEEP_LATE) and only its release is awaited.
// `phase`: where in the beacon interval it was submitted, by our clock and by the TSF (213-DRV-0091).
// `rate` and `length`: the send's rate (500 kb/s) and its frame's bytes, for the minute's counts (213-DRV-0104).
struct Pending{call:Option<wire::Call>,slot:usize,deadline:u64,grace_us:u64,unicast:bool,began:u64,phase:Phase,rate:u8,length:usize}
// 213-DRV-0091: how sends are held in the MAC. The first HELD_PROBES late frames of a run are probed, then one a minute;
// one frame released in time after the first is probed once beside them. A minute's releases by the beacon interval's
// phase, its late frames' outcomes and holds, and the frame buffer's full spells, for the minute line.
const HELD_PROBES:u32=8;
// 213-DRV-0091 step 2, a bench switch: true (as now) sets port 0 for INFRA once linked; false leaves it as before step 2.
const PORT_AS_LINUX:bool=true;
use data_mode::Phase;
#[path="detection_floor.rs"] pub(crate) mod detection_floor;
// 213-DRV-0102: the beacons of the joined BSS measured before its floor is written; whether the floor is due (a link or a
// recovery since), set, or refused for the band (said once).
const FLOOR_BEACONS:u32=8;
// When it alternates: Linux's values and those found before the first write, what this minute holds, whether the switch
// at the minute's boundary is still to be made (a send pending postpones it to the next poll), and switches that failed.
// `written`: what this minute's writes did not read back (every field is written, each read back on its own).
struct FloorState{enabled:bool,alternates:bool,due:bool,set:bool,refused:bool,linux:Option<detection_floor::Floor>,found:Option<[u32;4]>,
    kind:data_mode::FloorKind,switch_due:bool,failures:u32,written:data_mode::FloorWrite}
impl Default for FloorState{fn default()->Self{Self{enabled:detection_floor::DETECTION_FLOOR_AS_LINUX,alternates:detection_floor::DETECTION_FLOOR_ALTERNATES,
    due:false,set:false,refused:false,linux:None,found:None,kind:data_mode::FloorKind::None,switch_due:false,failures:0,written:Default::default()}}}
fn bin(us:Option<u64>)->Option<usize>{us.map(|us|((us/10_000)as usize).min(10))}
// 213-DRV-0104: `answered`: when the last send was answered or refused; `second`: when the current second of arrivals began
// and how many sends arrived in it.
struct TxWatch{probes:u32,minute_probed:bool,not_held:bool,held:Option<Event>,minute:data_mode::TxMinute,holds:[u16;32],holds_n:usize,
    full_since:Option<u64>,answered:Option<u64>,second:(u64,u32)}
impl Default for TxWatch{fn default()->Self{Self{probes:0,minute_probed:false,not_held:false,held:None,minute:Default::default(),holds:[0;32],
    holds_n:0,full_since:None,answered:None,second:(0,0)}}}
impl TxWatch{fn full_longest_ms_max(&mut self,ms:u64){self.minute.full_longest_ms=self.minute.full_longest_ms.max(ms);}
    // 213-DRV-0104: a send answered busy, counted for the minute line.
    fn busy(&mut self){self.minute.busy=self.minute.busy.saturating_add(1);self.answered=Some(time::monotonic_ns()/1_000_000);}
    // 213-DRV-0104: a send arrived at `now`: the time since the last answer, and the sends of this second.
    fn arrived(&mut self,now:u64){
        if let Some(at)=self.answered{let b=data_mode::ms_bin(now.saturating_sub(at));self.minute.gap_ms[b]=self.minute.gap_ms[b].saturating_add(1);}
        if self.second.1==0||now.saturating_sub(self.second.0)>=1000{self.second=(now,0);}
        self.second.1=self.second.1.saturating_add(1);self.minute.peak_sends=self.minute.peak_sends.max(self.second.1);
    }
    // 213-DRV-0104: a send of `length` bytes answered in time, `ms` after its arrival.
    fn in_time(&mut self,ms:u64,length:usize){
        let(c,b)=(data_mode::size_class(length),data_mode::ms_bin(ms).min(data_mode::MS_BINS-1));
        self.minute.answer_ms[c][b]=self.minute.answer_ms[c][b].saturating_add(1);
    }
    // 213-DRV-0104: a send to one station ended at `rate` (500 kb/s), acknowledged or not, after `attempts` tries.
    fn ended_at(&mut self,rate:u8,acked:bool,attempts:u8){
        let m=&mut self.minute;
        if let Some(i)=crate::txdesc::LEGACY_RATES.iter().position(|r|*r==rate){
            let r=&mut m.by_rate[i];r[0]=r[0].saturating_add(1);r[1]=r[1].saturating_add(u32::from(acked));r[2]=r[2].saturating_add(u32::from(attempts));
        }
        let t=usize::from(attempts.min(15));m.by_tries[t]=m.by_tries[t].saturating_add(1);
    }}
struct Waiter{call:wire::Call,deadline:u64}
struct Frames{bytes:[[u8;FRAME_BYTES];FRAMES],lengths:[usize;FRAMES],head:usize,count:usize}
impl Frames{
    fn new()->Self{Self{bytes:[[0;FRAME_BYTES];FRAMES],lengths:[0;FRAMES],head:0,count:0}}
    fn clear(&mut self){self.head=0;self.count=0;}
}

/// Receive slots 9 and 10 must be reserved and empty; the endpoint must be granted only to the WLAN station.
/// The borrowed data owner and all DMA remain alive after stop, failure, or dropping this adapter.
pub(crate) struct Service<'s,'q,'memory,'p,'a,'command,'receive,B:DeviceIo>{
    owner:Option<&'s mut DataSession<'q,'memory,'p,'a,'command,'receive,B>>,
    pending:Option<Pending>,waiters:[Option<Waiter>;WAITERS],frames:Frames,last_ms:u64,failure:Option<Fatal>,
    // The EIRP the channel was admitted under and the settings taken so far; `refused`: a country it cannot keep, so
    // nothing is sent (wlanhw 1.2). `event`: one for the caller's log.
    country:Option<(i8,u64)>,refused:bool,event:Option<Event>,strict:bool,
    // `late`: KEEP_LATE; `lates`: the sends that outran their time so far; `parked`: a late send parked on the ring
    // (SEND_BEHIND_LATE, `behind`), its release awaited.
    late:bool,lates:u32,parked:Option<Pending>,behind:bool,
    // SEND_BEHIND_LATE_ALTERNATES: whether it alternates, what this minute holds, and whether a switch is due.
    behind_alternates:bool,behind_kind:data_mode::FloorKind,behind_switch_due:bool,
    // DEBUG (2026-11-08): the SNAPSHOT registers at the start, and when each first changed (ms from the start, its value,
    // whether a send was pending), read once a step.
    first:data_mode::Snapshot,changes:Changes,start_ms:u64,
    // 801-DRV-0071: the Wi-Fi frames counted and when they last moved, the sends finished and acknowledged since, and the
    // unicast sends not acknowledged in a row (how many, since when, whether logged).
    heard:(u64,u64),quiet:(u32,u32),unacked:(u32,u64,bool),
    // When the data mode's checks were last made (CHECK_MS).
    checked:Option<u64>,
    // 213-DRV-0087: on a listen-only channel, the access point admitted for and when its beacon was last heard.
    listen:Option<([u8;6],u64)>,
    // 213-DRV-0087: the frames kept for the station, and when the reports' counts were last logged, as they were then.
    kept:u64,signals_at:(u64,crate::ppdu::Signals),
    // 213-DRV-0090: the data mode's count of the joined BSS's beacons when it last moved, then, and the counts then;
    // whether one was heard since the filter; the hole open since it; the holes so far and the longest; the sends
    // answered and acknowledged; minutes logged; a line that waits for the event slot.
    // The same for every Wi-Fi frame, the holes' measure while no beacon of the BSS reaches the host.
    beacons:(u64,u64,Marks),heard_bss:bool,frames_seen:(u64,u64,Marks),hole:Option<Hole>,holes:(u32,u64),sent:(u64,u64),minutes:u32,
    later:Option<Event>,
    // 213-DRV-0091: the link the firmware was told of: None, or the AID it was told; and a change still to tell it (the
    // AID of an association response, or None after a deauthentication), done when no send is pending.
    linked:Option<u16>,link_change:Option<Option<u16>>,links:u32,
    // 213-DRV-0091 step 2: port 0 set for INFRA, and its change due at a time: a beacon interval and a millisecond after the
    // H2Cs, as Linux's rtw89_mac_port_cfg_func_sw waits before it turns the port's function off.
    port_linked:bool,port_due:Option<(u64,bool)>,
    tx_watch:TxWatch,floor:FloorState,
}
/// The counts a hole's lines are told against (213-DRV-0090).
#[derive(Clone,Copy,Default)]
struct Marks{wifi:u64,kept:u64,ppdu:u64,pressure:u64,sent:u64,acked:u64}
// An open hole: its number, its last beacon's (or frame's) time, the counts when it was seen, the first after, and
// whether beacons measure it.
#[derive(Clone,Copy)]
struct Hole{n:u32,since:u64,marks:Marks,back:Option<u64>,by_beacons:bool}
/// On a listen-only channel a send goes to the admitted access point alone, while its beacon was heard within this, half
/// the station's own silence; after it, and before the beacon comes back, nothing is sent there.
pub(crate) const LISTEN_BEACON_MS:u64=QUIET_MS/2;
/// When each SNAPSHOT register first changed in data mode: (ms from the start, value, a send pending).
#[derive(Clone,Copy)]
pub(crate) struct Changes(pub [Option<(u64,u32,bool)>;data_mode::SNAPSHOT.len()]);
impl core::fmt::Debug for Changes{
    fn fmt(&self,f:&mut core::fmt::Formatter<'_>)->core::fmt::Result{
        for(at,change)in data_mode::SNAPSHOT.iter().zip(self.0){
            if let Some((ms,value,pending))=change{write!(f,"{:X}={:08X}@{}MS{} ",at,value,ms,if pending{"+TX"}else{""})?;}
        }Ok(())
    }
}
pub(crate) struct Stopped<'s,'q,'memory,'p,'a,'command,'receive,B:DeviceIo>{
    pub owner:&'s mut DataSession<'q,'memory,'p,'a,'command,'receive,B>,
    pub failure:Option<Fatal>,pub shutdown:data_mode::Shutdown,pub changes:Changes,
}
fn address(value:u64)->[u8;6]{core::array::from_fn(|i|(value>>(40-8*i))as u8)}
fn packed(value:[u8;6])->u64{value.into_iter().fold(0,|word,byte|(word<<8)|u64::from(byte))}
fn error(failure:&data_mode::Failure)->wlanhw::Error{
    match failure.cause{
        data_mode::Cause::Tx(tx::Error::Busy)=>wlanhw::Error::Busy,
        data_mode::Cause::Tx(tx::Error::Input(_))=>wlanhw::Error::Invalid,
        _=>wlanhw::Error::Failed,
    }
}

impl<'s,'q,'memory,'p,'a,'command,'receive,B:DeviceIo> Service<'s,'q,'memory,'p,'a,'command,'receive,B>{
    pub(crate) fn new(owner:&'s mut DataSession<'q,'memory,'p,'a,'command,'receive,B>)->Result<Self,sys::Error>{
        if RECEIVE.into_iter().any(|slot|dev::cap_info(slot).0!=mind::abi::CAP_KIND_NONE){return Err(sys::Error::NoSlot)}
        let first=owner.registers();let start_ms=time::monotonic_ns()/1_000_000;
        Ok(Self{owner:Some(owner),pending:None,waiters:[const{None};WAITERS],frames:Frames::new(),last_ms:start_ms,failure:None,
            country:None,refused:false,event:None,strict:!KEEP_SENDING,late:KEEP_LATE,lates:0,parked:None,behind:SEND_BEHIND_LATE,
            behind_alternates:SEND_BEHIND_LATE_ALTERNATES,behind_kind:data_mode::FloorKind::None,behind_switch_due:false,first,changes:Changes([None;data_mode::SNAPSHOT.len()]),start_ms,
            heard:(0,start_ms),quiet:(0,0),unacked:(0,0,false),checked:None,listen:None,kept:0,signals_at:(start_ms,Default::default()),
            beacons:(0,start_ms,Marks::default()),heard_bss:false,frames_seen:(0,start_ms,Marks::default()),hole:None,holes:(0,0),sent:(0,0),minutes:0,later:None,
            linked:None,link_change:None,links:0,port_linked:false,port_due:None,tx_watch:TxWatch::default(),floor:FloorState::default()})
    }
    /// The channel was admitted at `eirp_half_dbm` under the station's country, after `settings` set-country requests.
    pub(crate) fn under(mut self,eirp_half_dbm:i8,settings:u64)->Self{self.country=Some((eirp_half_dbm,settings));self}
    /// The channel is listen-only and was admitted for `bssid`, whose beacon was heard at `heard_ms` (213-DRV-0087).
    pub(crate) fn listening_to(mut self,bssid:[u8;6],heard_ms:u64)->Self{self.listen=Some((bssid,heard_ms));self}
    #[cfg(test)]
    pub(crate) fn take_event(&mut self)->Option<Event>{self.event.take()}
    #[cfg(test)]
    pub(crate) fn take_recovery(&mut self)->Option<data_mode::Recovery>{self.data().take_recovery()}
    #[cfg(test)]
    pub(crate) fn take_unreadable(&mut self)->Option<data_mode::Unreadable>{self.data().take_unreadable()}
    #[cfg(test)]
    pub(crate) fn take_firmware_error(&mut self)->Option<data_mode::FirmwareError>{self.data().take_firmware_error()}
    #[cfg(test)]
    pub(crate) fn take_held(&mut self)->Option<Event>{self.tx_watch.held.take()}
    #[cfg(test)]
    pub(crate) fn floor_as_linux(&mut self,on:bool,alternates:bool){self.floor.enabled=on;self.floor.alternates=alternates}
    #[cfg(test)]
    pub(crate) fn floor_registers(&mut self)->[u32;4]{self.data().floor_registers()}
    #[cfg(test)]
    pub(crate) fn data_held(&mut self)->data_mode::Held{self.data().held()}
    #[cfg(test)]
    pub(crate) fn take_halt_status(&mut self)->Option<data_mode::HaltStatus>{self.data().take_halt_status()}
    #[cfg(test)]
    pub(crate) fn take_ser_step(&mut self)->Option<data_mode::SerStep>{self.data().take_ser_step()}
    /// Whether a country that refuses the channel stops sending (wlanhw 1.2), or is only logged (KEEP_SENDING).
    #[cfg(test)]
    pub(crate) fn strict(mut self,on:bool)->Self{self.strict=on;self}
    /// Whether a send that outruns its time is answered and its release awaited (KEEP_LATE), or stops the service.
    #[cfg(test)]
    pub(crate) fn keep_late(mut self,on:bool)->Self{self.late=on;self}
    /// Whether the next send goes on the ring behind a late one (SEND_BEHIND_LATE).
    #[cfg(test)]
    pub(crate) fn send_behind_late(mut self,on:bool)->Self{self.behind=on;self}
    /// Whether SEND_BEHIND_LATE alternates by the minute (SEND_BEHIND_LATE_ALTERNATES).
    #[cfg(test)]
    pub(crate) fn send_behind_late_alternates(mut self,on:bool)->Self{self.behind_alternates=on;self}
    /// The data queues' frames final on DMA and the RPQ (tx.rs RPQ_ALONE), as on the Ryzen.
    #[cfg(test)]
    pub(crate) fn rpq_alone(&mut self){self.data().rpq_alone()}
    fn info(&mut self)->wlanhw::Info{
        let mut channels=List::default();channels.push(self.data().channel());
        // Its own channels: 1 to 13 on 2.4 GHz and the 5 GHz ones at 20 MHz (wlanhw.wit 2.0, 550-NET-0059, 213-DRV-0087).
        let mut supported=List::default();for ch in band::SUPPORTED{supported.push(ch);}
        let bands=mind::network::BAND_2GHZ|mind::network::BAND_2GHZ_OFDM|mind::network::BAND_5GHZ;
        wlanhw::Info{mac:packed(self.data().mac()),bands,
            channels,supported,frame_max:frames::FRAME_MAX as u16,channel:self.data().channel()}
    }
    // A new country keeps the channel if it still lets the radio send there at no less EIRP than it was admitted under.
    fn country(&mut self,code:&str,limits:&[wlanhw::Limit])->Result<(),wlanhw::Error>{
        let channel=self.data().channel();
        let Some((eirp,settings))=self.country.as_mut()else{return Err(wlanhw::Error::Failed)};
        *settings=settings.saturating_add(1);
        match Limits::accept(code,*settings,limits.iter().map(|l|(l.channel,l.eirp,l.listen))){
            Ok(taken)=>{
                // A listen-only channel admitted for an access point heard there stays so while the country keeps it.
                let listening=self.listen.is_some();
                let kept=taken.applied(channel).is_some_and(|a|(!a.listen||listening)&&a.eirp_half_dbm>=*eirp);
                self.event=Some(Event::DataCountry{code:taken.code(),channel,kept,enforced:self.strict});
                if kept||!self.strict{Ok(())}else{Err(wlanhw::Error::Failed)}
            }
            Err(Refusal::Invalid)=>Err(wlanhw::Error::Invalid),
            Err(Refusal::Failed)=>Err(wlanhw::Error::Failed),
        }
    }
    fn data(&mut self)->&mut DataSession<'q,'memory,'p,'a,'command,'receive,B>{self.owner.as_deref_mut().unwrap()}
    fn fail(&mut self,cause:Fatal){
        if self.failure.is_none(){self.watch();self.failure=Some(cause);}
        self.data().shutdown();self.answer_pending();
    }
    fn answer_pending(&mut self){
        if let Some(call)=self.pending.take().and_then(|p|p.call){let _=wlanhw::reply_send(call,Err(wlanhw::Error::Failed));}
        for waiter in &mut self.waiters{if let Some(waiter)=waiter.take(){let _=wlanhw::reply_wait(waiter.call,0);}}
    }
    // `hold`: the last poll gave a receive packet, so another may follow: a frame whose PPDU status report may be next waits
    // for it in the inbox, as Linux's rx_queue holds a frame for its report (213-DRV-0087), until a poll finds none; the
    // next step's drain polls again, so it waits a step at most, and no poll is added for it.
    fn collect(&mut self,hold:bool)->Result<(),Fatal>{
        let data=self.owner.as_deref_mut().unwrap();
        for _ in 0..FRAMES{
            if self.frames.count==FRAMES||hold&&data.awaits_signal(){break}
            let Some(record)=data.packet()else{break};
            if record.metadata().channel!=rx::Channel::Rxq||record.metadata().packet.kind!=rxdesc::TYPE_WIFI{return Err(Fatal::RetainedControl)}
            let frame=frames::from_packet(record.metadata().packet,record.buffer()).map_err(Fatal::Frame)?;
            if let Some((bssid,heard))=self.listen.as_mut(){
                let m=frame.mpdu();
                if m.len()>=24&&matches!(m[0],0x80|0x50)&&m[16..22]==*bssid{*heard=time::monotonic_ns()/1_000_000;}
            }
            // 213-DRV-0091: the access point's (re)association response to this station with status 0 carries the AID the
            // firmware is told of; its deauthentication or disassociation of this station ends the link.
            let(m,f)=(frame.mpdu(),data.current_filter());
            if m.len()>=24&&f.bssid()!=[0;6]&&m[4..10]==f.own()&&m[16..22]==f.bssid(){
                match m[0]{
                    0x10|0x30 if m.len()>=30&&m[26..28]==[0,0]=>{self.link_change=Some(Some(u16::from_le_bytes([m[28],m[29]])&0x3fff));}
                    0xa0|0xc0 if self.linked.is_some()=>{self.link_change=Some(None);}
                    _=>{}
                }
            }
            // 213-DRV-0104: a frame for this station, by the access point's rate in its descriptor and its retry bit.
            if m.len()>=24&&m[4..10]==f.own(){
                let mm=&mut self.tx_watch.minute;
                if let Some((_,rate))=crate::ppdu::key(record.buffer()){let b=data_mode::rx_rate_bin(rate);mm.rx_rate[b]=mm.rx_rate[b].saturating_add(1);}
                if m[1]&8!=0{mm.rx_retry=mm.rx_retry.saturating_add(1);}
            }
            let at=(self.frames.head+self.frames.count)%FRAMES;let signal=record.signal();
            let length=frame.write(data.channel(),data.current_filter().all_beacons(),&mut self.frames.bytes[at]).map_err(Fatal::Frame)?;
            self.frames.bytes[at][frames::SIGNAL]=signal;self.kept=self.kept.saturating_add(1);
            self.frames.lengths[at]=length;self.frames.count+=1;data.consume_packet();
        }Ok(())
    }
    fn progress(&mut self,now:u64)->Result<(),Fatal>{
        let grace_ms=|p:&Pending,data:&DataSession<'q,'memory,'p,'a,'command,'receive,B>|data.grace_us().saturating_sub(p.grace_us)/1000;
        if self.pending.as_ref().is_some_and(|p|now>=p.deadline.saturating_add(grace_ms(p,self.owner.as_deref().unwrap()))){
            if !self.late||self.pending.as_ref().is_some_and(|p|p.call.is_none()){return Err(Fatal::TransmitTimeout(self.data().transmit_seen()))}
            let seen=self.data().transmit_seen();let pending=self.pending.as_mut().unwrap();
            pending.deadline=pending.deadline.saturating_add(LATE_MS);let call=pending.call.take().unwrap();
            let attempts=u8::try_from(seen.tries).unwrap_or(15).clamp(1,15);
            let unicast=pending.unicast;
            let _=wlanhw::reply_send(call,Ok(&late_answer(attempts)));
            self.data().extend_send(LATE_MS*1000);self.lates=self.lates.saturating_add(1);
            self.tx_watch.minute.answered_late=self.tx_watch.minute.answered_late.saturating_add(1);
            if self.event.is_none(){self.event=Some(Event::LateFrame{count:self.lates,finished:seen.finished,fetched:seen.fetched,
                released:seen.released,length:seen.length,age_us:seen.age_us})}
            self.ended(unicast,false,now);
            // 213-DRV-0091: what holds it, probed while the MAC still has it.
            let w=&mut self.tx_watch;
            let probe=if w.probes<HELD_PROBES{w.probes+=1;true}else if !w.minute_probed{w.minute_probed=true;true}else{false};
            if probe{
                let(began,submitted)=self.pending.as_ref().map_or((now,Phase::default()),|p|(p.began,p.phase));let at=self.phase();
                let registers=self.data().held();
                self.tx_watch.held=Some(Event::Held{n:self.lates,held:true,ms:now.saturating_sub(began),submitted,at,registers});
            }
        }
        // 213-DRV-0104: a parked frame not released in LATE_MS stops the service, as a late one does.
        if self.parked.as_ref().is_some_and(|p|now>=p.deadline.saturating_add(grace_ms(p,self.owner.as_deref().unwrap()))){
            return Err(Fatal::TransmitTimeout(self.data().transmit_seen()))}
        if self.pending.is_some(){self.finish_send()?}
        if self.parked.is_some(){self.finish_parked()?}
        // 213-DRV-0091: the frame buffer's full spells.
        let full=self.frames.count==FRAMES;let w=&mut self.tx_watch;
        match(full,w.full_since){
            (true,None)=>{w.full_since=Some(now);w.minute.full_times=w.minute.full_times.saturating_add(1);}
            (false,Some(since))=>{w.full_longest_ms_max(now.saturating_sub(since));w.full_since=None;}
            _=>{}
        }
        // The receive queue is drained by the receive alone. With the buffer full it is left unread, its packets waiting
        // in the ring, and the release reports are still read.
        let mut empty=false;
        for _ in 0..DRAIN{
            let full=self.frames.count==FRAMES;
            let received=self.data().poll_more(!full,WORK_US).map_err(Fatal::Radio)?;self.collect(received[0])?;
            if received==[false,false]{empty=!full;break}
            if full{break}
        }
        // The checks and the coexistence reconcile follow every CHECK_MS, once the queue was read empty: the reconcile
        // reads that queue to its end, and of the frames it finds for this station only four have a slot; the rest it
        // drops. CHECK_LATEST_MS without them, they are made over a queue that is not empty.
        let since=now.saturating_sub(self.checked.unwrap_or(self.start_ms));
        if (self.checked.is_none()||since>=CHECK_MS)&&(empty||since>=CHECK_LATEST_MS){
            self.checked=Some(now);let received=self.data().poll_packets(WORK_US).map_err(Fatal::Radio)?;self.collect(received[0])?;
        }
        // A release the drain took is acted on in this step, not the next one after its receive wait.
        if self.pending.is_some(){self.finish_send()?}
        if self.parked.is_some(){self.finish_parked()?}
        self.switch_behind();
        self.tell_link()?;self.set_floor();
        self.listened(now)?;self.holes(now);self.signals(now);
        let waiting=self.frames.count as u32;
        for waiter in &mut self.waiters{
            if waiter.as_ref().is_some_and(|w|waiting!=0||now>=w.deadline){
                let _=wlanhw::reply_wait(waiter.take().unwrap().call,waiting);
            }
        }
        Ok(())
    }
    fn finish_send(&mut self)->Result<(),Fatal>{
        if let Some(done)=self.data().advance(WORK_US).map_err(Fatal::Radio)?{
            let tx::AttemptCount::Known(attempts)=done.attempts else{return Err(Fatal::UnknownAttempts)};
            if !(1..=15).contains(&attempts){return Err(Fatal::UnknownAttempts)}
            let pending=self.pending.take().unwrap();let now=time::monotonic_ns()/1_000_000;
            let at=self.release_phase();
            if pending.call.is_some()&&self.tx_watch.probes>0&&!self.tx_watch.not_held{
                self.tx_watch.not_held=true;let registers=self.data().held();
                self.tx_watch.held=Some(Event::Held{n:0,held:false,ms:now.saturating_sub(pending.began),submitted:pending.phase,at,registers});
            }
            if pending.unicast{self.tx_watch.ended_at(pending.rate,done.acked,attempts);}
            if let Some(call)=pending.call{let _=wlanhw::reply_send(call,Ok(&wlanhw::Sent{acked:done.acked,attempts}));
                self.tx_watch.in_time(now.saturating_sub(pending.began),pending.length);self.ended(pending.unicast,done.acked,now);}
            else{self.late_released(pending.began,done.acked,attempts,now);}
        }Ok(())
    }
    // 213-DRV-0104: the parked frame's release, after the pending one's in a step, counted as a late frame's.
    fn finish_parked(&mut self)->Result<(),Fatal>{
        let Some(done)=self.data().parked_release(WORK_US).map_err(Fatal::Radio)? else{return Ok(())};
        let tx::AttemptCount::Known(attempts)=done.attempts else{return Err(Fatal::UnknownAttempts)};
        if !(1..=15).contains(&attempts){return Err(Fatal::UnknownAttempts)}
        let parked=self.parked.take().unwrap();let now=time::monotonic_ns()/1_000_000;
        let m=&mut self.tx_watch.minute;m.parked_released=m.parked_released.saturating_add(1);if done.acked{m.parked_acked=m.parked_acked.saturating_add(1);}
        if parked.unicast{self.tx_watch.ended_at(parked.rate,done.acked,attempts);}
        self.release_phase();self.late_released(parked.began,done.acked,attempts,now);Ok(())
    }
    // 213-DRV-0104, SEND_BEHIND_LATE_ALTERNATES: off for the first whole minute, then on, and so on; at a poll with no send
    // pending or parked once the minute's line is taken.
    fn switch_behind(&mut self){
        use data_mode::FloorKind;
        if !self.behind_alternates||!self.behind_switch_due||self.pending.is_some()||self.parked.is_some(){return}
        self.behind_kind=if self.behind_kind==FloorKind::Off{FloorKind::On}else{FloorKind::Off};self.behind_switch_due=false;
    }
    // Whether a send is put behind a late one now.
    fn behind_now(&self)->bool{if self.behind_alternates{self.behind_kind==data_mode::FloorKind::On}else{self.behind}}
    // 213-DRV-0091: the release's phase in the beacon interval, by our clock and by the TSF, into the minute's bins.
    fn release_phase(&mut self)->Phase{
        let at=self.phase();
        for(bins,us)in[(&mut self.tx_watch.minute.clock,at.clock_us),(&mut self.tx_watch.minute.tsf,at.tsf_us)]{
            if let Some(bin)=bin(us){bins[bin]=bins[bin].saturating_add(1);}
        }
        at
    }
    // A late send was counted when it was answered (KEEP_LATE); its release goes into the minute's late frames and holds,
    // and how long it took in the end is logged.
    fn late_released(&mut self,began:u64,acked:bool,attempts:u8,now:u64){
        let m=&mut self.tx_watch.minute;m.late=m.late.saturating_add(1);
        if acked&&attempts==1{m.first_try=m.first_try.saturating_add(1);}if !acked{m.not_acked=m.not_acked.saturating_add(1);}
        let w=&mut self.tx_watch;if w.holds_n<w.holds.len(){w.holds[w.holds_n]=u16::try_from(now.saturating_sub(began)).unwrap_or(u16::MAX);w.holds_n+=1;}
        if self.event.is_none(){self.event=Some(Event::LateRelease{ms:now.saturating_sub(began),attempts,acked})}
    }
    // 213-DRV-0091: a link change told to the firmware once no send is pending; a busy queue waits a step, a failure that
    // shut the data mode down ends the service.
    fn tell_link(&mut self)->Result<(),Fatal>{
        self.set_port()?;
        let Some(change)=self.link_change else{return Ok(())};
        if self.pending.is_some()||self.parked.is_some(){return Ok(())}
        if change==self.linked{self.link_change=None;return Ok(())}
        // The registers just before and after, the first two times (213-DRV-0090's probe).
        let probed=self.links<2;let before=probed.then(||self.data().probe());
        match self.data().connect(change,COMMAND_US){
            Ok(())=>{
                self.link_change=None;let was=self.linked;self.linked=change;self.links=self.links.saturating_add(1);
                let(channel,bssid)=(self.data().channel(),self.data().current_filter().bssid());
                let after=probed.then(||self.data().probe());
                let event=Event::Linked{channel,bssid,aid:change,was,before,after};
                let now=time::monotonic_ns()/1_000_000;
                if change.is_some(){self.floor.due=true;}
                if PORT_AS_LINUX&&change.is_some()!=self.port_linked{self.port_due=Some((now.saturating_add(PORT_WAIT_MS),change.is_some()));}
                if self.event.is_none(){self.event=Some(event)}else{self.later=Some(event)}Ok(())
            }
            Err(failure) if failure.shutdown.is_some()=>Err(Fatal::Radio(failure)),
            Err(_)=>Ok(()),
        }
    }
    // 213-DRV-0091 step 2: the port's change once it is due and no send is pending.
    fn set_port(&mut self)->Result<(),Fatal>{
        let Some((due,linked))=self.port_due else{return Ok(())};
        if self.pending.is_some()||self.parked.is_some()||time::monotonic_ns()/1_000_000<due{return Ok(())}
        match self.data().port_link(linked,COMMAND_US){
            Ok(())=>{
                self.port_due=None;self.port_linked=linked;
                let after=(self.links<=2).then(||self.data().probe());let event=Event::Port{channel:self.data().channel(),linked,after};
                if self.event.is_none(){self.event=Some(event)}else{self.later=Some(event)}Ok(())
            }
            Err(failure) if failure.shutdown.is_some()=>Err(Fatal::Radio(failure)),
            Err(_)=>Ok(()),
        }
    }
    // 213-DRV-0091: where in the beacon interval now is: by our clock since the last beacon taken from the ring, and by the
    // port's TSF; the interval as the last beacon gave it.
    fn phase(&mut self)->Phase{
        let c=self.data().counts();let interval=u64::from(c.beacon_interval_us);if interval==0{return Phase::default()}
        let now_us=time::monotonic_ns()/1000;
        let clock_us=(c.beacon_at_us!=0&&now_us>=c.beacon_at_us).then(||(now_us-c.beacon_at_us)%interval);
        Phase{clock_us,tsf_us:self.data().tsf().map(|tsf|tsf%interval),interval_us:c.beacon_interval_us}
    }
    // 213-DRV-0102: the detection floor, once due and the joined BSS's beacons measured, on 2.4 GHz only; no send pending.
    fn set_floor(&mut self){
        if !self.floor.enabled||self.pending.is_some()||self.parked.is_some(){return}
        if self.floor.alternates&&self.floor.switch_due{self.switch_floor();}
        if !self.floor.due{return}
        let channel=self.data().channel();
        if channel>14{
            self.floor.due=false;
            if !self.floor.refused{self.floor.refused=true;self.floor_event(Event::FloorRefused{channel});}return
        }
        let c=self.data().counts();if c.bss_measured<FLOOR_BEACONS{return}
        let Some(dbm)=c.bss_dbm() else{return};let floor=detection_floor::for_rssi(dbm);
        if self.floor.alternates{
            // The values found are read once, before any write; a link or a recovery writes this minute's kind again.
            self.floor.due=false;
            if let(Some(linux),Some(found))=(self.floor.linux,self.floor.found){
                let fields=if self.floor.kind==data_mode::FloorKind::On{detection_floor::fields(&linux)}else{detection_floor::restore(&linux,&found)};
                let result=self.data().write_floor(&fields);self.floor_written(result);return
            }
            let found=self.data().floor_registers();self.floor.found=Some(found);self.floor.linux=Some(floor);
            self.floor_event(Event::FloorAlternates{channel,dbm,measured:c.bss_measured,ofdm_pd:floor.ofdm_pd,cck_pd:floor.cck_pd,edcca:floor.edcca,found});
            return
        }
        let result=self.data().write_floor(&detection_floor::fields(&floor));let written=result.is_ok_and(|w|w.unread==0);
        self.floor.due=false;self.floor.set=result.is_ok();self.floor_written(result);
        let registers=self.data().floor_registers();
        self.floor_event(Event::FloorSet{channel,dbm,measured:c.bss_measured,ofdm_pd:floor.ofdm_pd,cck_pd:floor.cck_pd,edcca:floor.edcca,written,registers});
    }
    // 213-DRV-0102, DETECTION_FLOOR_ALTERNATES: at a minute's boundary, off first, then Linux's, each field read back.
    fn switch_floor(&mut self){
        use data_mode::FloorKind;
        let(Some(linux),Some(found))=(self.floor.linux,self.floor.found) else{self.floor.switch_due=false;return};
        let next=if self.floor.kind==FloorKind::Off{FloorKind::On}else{FloorKind::Off};
        let fields=match(self.floor.kind,next){
            (_,FloorKind::On)=>Some(detection_floor::fields(&linux)),
            (FloorKind::On,_)=>Some(detection_floor::restore(&linux,&found)),
            _=>None,
        };
        if let Some(fields)=fields{
            let result=self.data().write_floor(&fields);if result.is_ok()&&next==FloorKind::On{self.floor.set=true;}self.floor_written(result);
        }
        self.floor.kind=next;self.floor.switch_due=false;
    }
    // A write's registers not read back are the minute's to say; an access fault is counted.
    fn floor_written(&mut self,result:Result<data_mode::FloorWrite,crate::io::Fault>){
        match result{
            Ok(w)=>{let s=&mut self.floor.written;s.unread|=w.unread;s.unchanged|=w.unchanged;}
            Err(_)=>{self.floor.failures=self.floor.failures.saturating_add(1);}
        }
    }
    fn floor_event(&mut self,event:Event){if self.event.is_none(){self.event=Some(event)}else{self.later=Some(event)}}
    // 213-DRV-0102: the minute's PPDU reports by signal, taken, and the floor's registers now.
    fn floor_minute(&mut self)->data_mode::FloorMinute{
        let(ppdu_rssi,no_rpl,unparsed,header_over)=self.data().take_ppdu_rssi();let c=self.data().counts();
        let minute=data_mode::FloorMinute{ppdu_rssi,unparsed,registers:self.data().floor_registers(),bss_dbm:c.bss_dbm(),measured:c.bss_measured,
            set:self.floor.set,kind:self.floor.kind,switch_failures:core::mem::take(&mut self.floor.failures),written:core::mem::take(&mut self.floor.written),
            no_rpl,header_over};
        // The next minute holds the other kind, switched at the next poll with no send pending.
        if self.floor.alternates&&self.floor.linux.is_some(){self.floor.switch_due=true;}
        minute
    }
    // 213-DRV-0091: the minute's sends, taken: the next minute starts at zero, a full spell going on counted to now.
    fn tx_minute(&mut self,now:u64)->data_mode::TxMinute{
        let w=&mut self.tx_watch;w.minute_probed=false;
        if let Some(since)=w.full_since{w.full_longest_ms_max(now.saturating_sub(since));}
        let holds=&mut w.holds[..w.holds_n];holds.sort_unstable();
        w.minute.median_hold_ms=holds.get(holds.len()/2).map_or(0,|h|u64::from(*h));w.holds_n=0;
        let(kind,fixed)=(self.behind_kind,self.behind&&!self.behind_alternates);
        let mut minute=core::mem::take(&mut self.tx_watch.minute);minute.behind=kind;minute.behind_fixed=fixed;
        // The next minute holds the other kind, switched at the next poll with nothing pending or parked.
        if self.behind_alternates{self.behind_switch_due=true;}
        minute
    }
    // A send answered: counted for a stall's line, and a unicast one not acknowledged into the run UNHEARD watches.
    fn ended(&mut self,unicast:bool,acked:bool,now:u64){
        self.tx_watch.answered=Some(now);
        self.quiet.0=self.quiet.0.saturating_add(1);if acked{self.quiet.1=self.quiet.1.saturating_add(1);}
        self.sent.0=self.sent.0.saturating_add(1);if acked{self.sent.1=self.sent.1.saturating_add(1);}
        if !unicast{return}
        if acked{self.unacked=(0,0,false);return}
        if self.unacked.0==0{self.unacked.1=now;}self.unacked.0=self.unacked.0.saturating_add(1);
        let(count,since,logged)=self.unacked;
        if !logged&&count>=UNHEARD&&now.saturating_sub(since)>=UNHEARD_MS&&self.event.is_none(){
            self.unacked.2=true;let heard=now.saturating_sub(self.heard.1);
            self.event=Some(Event::Unheard{channel:self.data().channel(),count,ms:now.saturating_sub(since),heard_ms:heard});
        }
    }
    // 801-DRV-0071: a receive that gives no Wi-Fi frame for QUIET_MS, while the filter lets a network's beacons in, stops the service.
    fn listened(&mut self,now:u64)->Result<(),Fatal>{
        let frames=self.data().counts().wifi;let watched=self.data().filter().is_some_and(|f|f.all_beacons()||f.bssid()!=[0;6]);
        if frames!=self.heard.0||!watched{self.heard=(frames,now);self.quiet=(0,0);return Ok(())}
        let quiet_ms=now.saturating_sub(self.heard.1);if quiet_ms<QUIET_MS{return Ok(())}
        let stall=Stall{quiet_ms,frames,sent:self.quiet.0,acked:self.quiet.1};
        if self.event.is_none(){self.event=Some(Event::Stalled{channel:self.data().channel(),quiet_ms,frames,sent:stall.sent,acked:stall.acked})}
        Err(Fatal::ReceiveStalled(stall))
    }
    // 213-DRV-0087: once in SIGNALS_MS while they changed, what became of the PPDU status reports since the admission;
    // 213-DRV-0090: and every SIGNALS_MS the traffic's line, after it.
    fn signals(&mut self,now:u64){
        if now.saturating_sub(self.signals_at.0)<SIGNALS_MS||self.event.is_some(){return}
        let signals=self.data().counts().signals;
        if signals!=self.signals_at.1{let crate::ppdu::Signals{measured,late,unmatched}=signals;
            self.event=Some(Event::Signals{channel:self.data().channel(),kept:self.kept,measured,late,unmatched});}
        self.signals_at=(now,signals);
        let probe=(self.minutes%PROBE_MINUTES==0).then(||self.data().probe());self.minutes=self.minutes.saturating_add(1);
        let(c,channel)=(self.data().counts(),self.data().channel());
        let bssid=self.data().filter().map_or([0;6],|f|f.bssid());
        let traffic=Event::Traffic{channel,wifi:c.wifi,kept:self.kept,filtered:c.filtered,invalid:c.invalid,pressure:c.pressure,
            ppdu:c.ppdu_reports,beacons:c.beacons,bssid,other_beacons:c.other_beacons,
            refused:(self.minutes==1&&c.refused.count!=0).then_some(c.refused),sent:self.sent.0,acked:self.sent.1,late:self.lates,holes:self.holes.0,longest_ms:self.holes.1,probe,
            errors:c.errors,polls:self.data().take_polls(),tx:self.tx_minute(now),floor:self.floor_minute()};
        if self.event.is_none(){self.event=Some(traffic)}else{self.later=Some(traffic)}
    }
    // The counts now, for a hole's lines.
    fn marks(&mut self)->Marks{
        let c=self.data().counts();Marks{wifi:c.wifi,kept:self.kept,ppdu:c.ppdu_reports,pressure:c.pressure,sent:self.sent.0,acked:self.sent.1}
    }
    // 213-DRV-0090: a hole opens when the joined BSS, heard once, sends no beacon for HOLE_MS, and closes with its next one;
    // while none of its beacons reaches the host, when no Wi-Fi frame at all comes for HOLE_MS, closed by the next.
    // Each end is logged with the counts since the other and, for the first HOLES_PROBED, the registers then. A line
    // that finds the event slot taken waits a step; a hole's length is told from its beacons or frames, not the lines.
    fn holes(&mut self,now:u64){
        if self.event.is_none(){self.event=self.later.take();}
        let c=self.data().counts();
        if c.beacons!=self.beacons.0{
            self.beacons=(c.beacons,now,self.marks());self.heard_bss=true;
            if let Some(hole)=self.hole.as_mut().filter(|h|h.by_beacons){hole.back.get_or_insert(now);}
        }
        if c.wifi!=self.frames_seen.0{
            self.frames_seen=(c.wifi,now,self.marks());
            if let Some(hole)=self.hole.as_mut().filter(|h|!h.by_beacons){hole.back.get_or_insert(now);}
        }
        let joined=self.data().filter().is_some_and(|f|!f.all_beacons()&&f.bssid()!=[0;6]);
        if !joined||!self.heard_bss&&self.frames_seen.0==0{self.hole=None;return}
        if self.event.is_some(){return}
        let d=|a:u64,b:u64|a.saturating_sub(b);
        let(by_beacons,last)=if self.heard_bss{(true,self.beacons)}else{(false,self.frames_seen)};
        let limit=if by_beacons{HOLE_MS}else{FRAMES_HOLE_MS};
        match self.hole{
            None if now.saturating_sub(last.1)>=limit=>{
                let n=self.holes.0.saturating_add(1);self.holes.0=n;let(was,mark,since)=(last.2,self.marks(),last.1);
                let probe=(n<=HOLES_PROBED).then(||self.data().probe());let bssid=self.data().current_filter().bssid();
                self.event=Some(Event::Hole{n,channel:self.data().channel(),bssid,by_beacons,ms:now.saturating_sub(since),wifi:d(mark.wifi,was.wifi),
                    kept:d(mark.kept,was.kept),ppdu:d(mark.ppdu,was.ppdu),pressure:d(mark.pressure,was.pressure),sent:d(mark.sent,was.sent),
                    acked:d(mark.acked,was.acked),pending:self.pending.is_some(),probe});
                self.hole=Some(Hole{n,since,marks:mark,back:None,by_beacons});
            }
            Some(Hole{n,since,marks:was,back:Some(back),..})=>{
                let mark=self.marks();let probe=(n<=HOLES_PROBED).then(||self.data().probe());let ms=back.saturating_sub(since);
                self.holes.1=self.holes.1.max(ms);
                self.event=Some(Event::HoleEnded{n,ms,wifi:d(mark.wifi,was.wifi),kept:d(mark.kept,was.kept),ppdu:d(mark.ppdu,was.ppdu),
                    pressure:d(mark.pressure,was.pressure),sent:d(mark.sent,was.sent),acked:d(mark.acked,was.acked),probe});
                self.hole=None;
            }
            _=>{}
        }
    }
    // DEBUG: the first change of each SNAPSHOT register, while the data mode runs: after its shutdown they are its own
    // (BB off, DMA stopped), which the Ryzen's log of 2026-11-08 had taken for the firmware's.
    fn watch(&mut self){
        let Some(data)=self.owner.as_deref_mut()else{return};if data.snapshot().is_some(){return}let now=data.registers();
        let ms=(time::monotonic_ns()/1_000_000).saturating_sub(self.start_ms);let pending=self.pending.is_some();
        for (i,change) in self.changes.0.iter_mut().enumerate(){
            if change.is_none()&&now.0[i]!=self.first.0[i]{*change=Some((ms,now.0[i],pending));}
        }
    }
    /// One receive and one bounded RX/TX progress step; continuous IPC or RX traffic cannot suppress the other.
    pub(crate) fn step(&mut self)->bool{
        if self.failure.is_some(){return false}
        self.watch();
        let slot=if self.pending.as_ref().is_some_and(|p|p.slot==RECEIVE[0]){RECEIVE[1]}else{RECEIVE[0]};
        let received=Endpoint::SERVICE.recv_timeout(slot,POLL_MS);
        let now=time::monotonic_ns()/1_000_000;
        if now<self.last_ms{self.fail(Fatal::Clock)}else{self.last_ms=now;if let Err(e)=self.progress(now){self.fail(e)}}
        let dispatch_now=time::monotonic_ns()/1_000_000;
        if dispatch_now<self.last_ms{self.fail(Fatal::Clock)}else{self.last_ms=dispatch_now}
        match received{
            Ok(request)=>self.dispatch(request,slot,dispatch_now),
            Err(sys::Error::Other(code))if code==mind::abi::ERR_TIMEOUT=>{},
            Err(error)=>self.fail(Fatal::Ipc(error)),
        }
        self.failure.is_none()
    }
    fn dispatch(&mut self,received:mind::ipc::Received,slot:usize,now:u64){
        let mut scratch=[0;wlanhw::REQUEST_MAX];
        let(request,call)=match wlanhw::decode(&received,slot,&mut scratch){
            Ok(decoded)=>decoded,Err(reason)=>{if received.is_call{let _=wire::reject(reason);}return},
        };
        if self.failure.is_some(){absent(request,call);return}
        match request{
            wlanhw::Request::Info=>{let info=self.info();let _=wlanhw::reply_info(call,Ok(&info));},
            wlanhw::Request::SetChannel{channel}=>{
                let result=if channel==self.data().channel(){Ok(())}else{Err(wlanhw::Error::Invalid)};
                let _=wlanhw::reply_set_channel(call,result);
            },
            wlanhw::Request::SetFilter{filter}=>{
                // 213-DRV-0091: another filter ends the link the firmware was told of, before its CAM entry is replaced.
                if self.linked.is_some()&&self.pending.is_none()&&self.parked.is_none(){
                    match self.data().connect(None,COMMAND_US).and_then(|()|if self.port_linked{self.data().port_link(false,COMMAND_US)}else{Ok(())}){
                        Ok(())=>{self.linked=None;self.link_change=None;self.port_linked=false;self.port_due=None;}
                        Err(failure) if failure.shutdown.is_some()=>{let _=wlanhw::reply_set_filter(call,Err(error(&failure)));self.fail(Fatal::Radio(failure));return}
                        Err(_)=>{}
                    }
                }
                let result=self.data().set_filter(filter.all_beacons,address(filter.bssid),address(filter.own),COMMAND_US);
                // 213-DRV-0090: another BSS's beacons are counted from none, and no hole is open.
                if result.is_ok(){self.frames.clear();self.beacons.1=now;self.heard_bss=false;self.hole=None;}
                let reply=result.as_ref().map(|_|()).map_err(error);let _=wlanhw::reply_set_filter(call,reply);
                if let Err(failure)=result{if failure.shutdown.is_some(){self.fail(Fatal::Radio(failure))}}
            },
            wlanhw::Request::Send{frame,rate,attempts}=>{
                self.tx_watch.arrived(now);
                if self.refused{let _=wlanhw::reply_send(call,Err(wlanhw::Error::Failed));return}
                // 5 GHz is OFDM alone: 6 to 54 Mb/s (wlanhw.wit `bands` bit 1).
                if self.data().channel()>14&&rate<12{let _=wlanhw::reply_send(call,Err(wlanhw::Error::Invalid));return}
                if let Some((bssid,heard))=self.listen{
                    let probe=frame.first().is_some_and(|c|c&0xfc==0x40);let to=frame.get(4..10).is_some_and(|r|r==bssid);
                    if probe||!to||now.saturating_sub(heard)>LISTEN_BEACON_MS{let _=wlanhw::reply_send(call,Err(wlanhw::Error::Failed));return}
                }
                // 213-DRV-0104: a late send, answered, is parked so that this one goes behind it (SEND_BEHIND_LATE).
                if self.pending.is_some(){
                    let late=self.pending.as_ref().is_some_and(|p|p.call.is_none());
                    if !(self.behind_now()&&late&&self.parked.is_none()&&self.data().park().is_ok()){self.tx_watch.busy();let _=wlanhw::reply_send(call,Err(wlanhw::Error::Busy));return}
                    self.parked=self.pending.take();self.tx_watch.minute.parked=self.tx_watch.minute.parked.saturating_add(1);
                }
                let Some(deadline)=now.checked_add(u64::from(COMMAND_US/1000)-FRAME_MARGIN_MS)else{
                    let _=wlanhw::reply_send(call,Err(wlanhw::Error::Failed));self.fail(Fatal::Clock);return};
                let mut call=call;
                if call.defer().is_err(){self.tx_watch.busy();let _=wlanhw::reply_send(call,Err(wlanhw::Error::Busy));return}
                let grace_us=self.data().grace_us();let unicast=frame.get(4).is_some_and(|byte|byte&1==0);
                let tries=if attempts<=15{attempts.min(SEND_TRIES)}else{attempts};
                match self.data().begin(frame,rate,tries,COMMAND_US){
                    Ok(())=>{
                        // KEEP_LATE: the queue's own budget never ends a frame first; the service's deadlines do. Within a step
                        // the 10 ms margin was not always enough (the Ryzen, 2026-11-08: Tx(Io(Timeout)) inside a drain).
                        if self.late{self.data().extend_send(LATE_MS*1000);}
                        let phase=self.phase();
                        self.pending=Some(Pending{call:Some(call),slot,deadline,grace_us,unicast,began:now,phase,rate,length:frame.len()})
                    },
                    Err(failure)=>{if error(&failure)==wlanhw::Error::Busy{self.tx_watch.busy();}let _=wlanhw::reply_send(call,Err(error(&failure)));
                        if failure.shutdown.is_some(){self.fail(Fatal::Radio(failure))}},
                }
            },
            wlanhw::Request::Receive=>{
                if self.frames.count==0{let _=wlanhw::reply_receive(call,Err(wlanhw::Error::Empty));return}
                let at=self.frames.head;let _=wlanhw::reply_receive(call,Ok(&self.frames.bytes[at][..self.frames.lengths[at]]));
                self.frames.head=(at+1)%FRAMES;self.frames.count-=1;
            },
            wlanhw::Request::Wait=>{
                let count=self.frames.count as u32;
                if count!=0{let _=wlanhw::reply_wait(call,count);return}
                let Some(deadline)=now.checked_add(WAIT_MS)else{let _=wlanhw::reply_wait(call,0);self.fail(Fatal::Clock);return};
                let mut call=call;
                match self.waiters.iter_mut().find(|entry|entry.is_none()){
                    Some(entry)if call.defer().is_ok()=>*entry=Some(Waiter{call,deadline}),
                    _=>{let _=wlanhw::reply_wait(call,0);},
                }
            },
            wlanhw::Request::Attach{..}=>{let _=wlanhw::reply_attach(call,Err(wlanhw::Error::Invalid));},
            wlanhw::Request::Kick=>{let _=wlanhw::reply_kick(call,Err(wlanhw::Error::Invalid));},
            // wlanhw 1.2 (550-NET-0035): `failed` leaves the radio sending nothing until a country it can keep.
            wlanhw::Request::SetCountry{code,limits,..}=>{
                let reply=self.country(code.as_str(),limits.as_slice());
                match reply{Ok(())=>self.refused=false,Err(wlanhw::Error::Failed)=>self.refused=self.strict,Err(_)=>{}}
                let info=self.info();let _=wlanhw::reply_set_country(call,reply.map(|()|&info));
            },
        }
    }
    pub(crate) fn stop(mut self)->Stopped<'s,'q,'memory,'p,'a,'command,'receive,B>{
        self.answer_pending();let owner=self.owner.take().unwrap();let shutdown=owner.shutdown();
        Stopped{owner,failure:self.failure.take(),shutdown,changes:self.changes}
    }
    pub(crate) fn run(mut self,events:&mut impl FnMut(Event))->Stopped<'s,'q,'memory,'p,'a,'command,'receive,B>{
        loop{
            let more=self.step();if let Some(event)=self.event.take(){events(event)}
            if let Some(r)=self.owner.as_deref_mut().and_then(|data|data.take_recovery()){
                // 213-DRV-0102: a recovery may have reset the BB; the floor is written again.
                if self.floor.set||self.floor.found.is_some(){self.floor.due=true;}
                events(Event::Recovered{count:r.count,ser:r.ser,cmac:r.cmac,bb_us:r.bb_us,restored:r.restored,waited_us:r.waited_us,after_reconcile:r.after_reconcile})}
            if let Some(u)=self.owner.as_deref_mut().and_then(|data|data.take_unreadable()){
                events(Event::Unreadable{count:u.count,register:u.register,value:u.value,reads:u.reads,waited_us:u.waited_us,
                    ser_before:u.ser_before,ser_after:u.ser_after,recovered:u.recovered})}
            if let Some(held)=self.tx_watch.held.take(){events(held)}
            if let Some(s)=self.owner.as_deref_mut().and_then(|data|data.take_ser_step()){
                events(Event::SerStep{count:s.count,from:s.from,to:s.to,since_us:s.since_us,polls:s.polls})}
            if let Some(h)=self.owner.as_deref_mut().and_then(|data|data.take_halt_status()){
                events(Event::HaltStatus{count:h.count,hisr:h.hisr,ctrl:h.ctrl,ser:h.ser,intx:h.intx})}
            while let Some(e)=self.owner.as_deref_mut().and_then(|data|data.take_firmware_error()){
                events(Event::FirmwareError{count:e.count,ctrl:e.ctrl,raw:e.raw,ser:e.ser,code:e.code,name:e.name,level:e.level,event:e.event})}
            if !more{break}
        }self.stop()
    }
}
impl<B:DeviceIo> Drop for Service<'_,'_,'_,'_,'_,'_,'_,B>{
    fn drop(&mut self){self.answer_pending();if let Some(owner)=self.owner.as_deref_mut(){owner.shutdown();}}
}
fn absent(request:wlanhw::Request<'_>,call:wire::Call){
    let no=wlanhw::Error::NoDevice;
    let _=match request{
        wlanhw::Request::Info=>wlanhw::reply_info(call,Err(no)),
        wlanhw::Request::SetChannel{..}=>wlanhw::reply_set_channel(call,Err(no)),
        wlanhw::Request::SetFilter{..}=>wlanhw::reply_set_filter(call,Err(no)),
        wlanhw::Request::Send{..}=>wlanhw::reply_send(call,Err(no)),
        wlanhw::Request::Receive=>wlanhw::reply_receive(call,Err(no)),
        wlanhw::Request::Wait=>wlanhw::reply_wait(call,0),
        wlanhw::Request::Attach{..}=>wlanhw::reply_attach(call,Err(no)),
        wlanhw::Request::Kick=>wlanhw::reply_kick(call,Err(no)),
        wlanhw::Request::SetCountry{..}=>wlanhw::reply_set_country(call,Err(no)),
    };
}
