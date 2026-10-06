//! idl/gpio.wit as `tool::Gpio`: the client the shell (or `wm`, `console`) lends in SLOT_GPIO.
use crate::tool::{Controller, Gpio, Pin, Pull, Refusal};
use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::SLOT_GPIO;
use mind::idl::gpio as idl;
use mind::ipc::Endpoint;

pub struct Service(Endpoint);

impl Service {
    pub fn new() -> Self { Self(Endpoint(SLOT_GPIO)) }
}

impl Default for Service {
    fn default() -> Self { Self::new() }
}

fn lost(error: mind::Error) -> Refusal { Refusal::Lost(alloc::format!("{:?}", error)) }
fn refused(error: idl::Error) -> Refusal {
    match error { idl::Error::NoController => Refusal::NoController, idl::Error::NoPin => Refusal::NoPin, idl::Error::Reserved => Refusal::Reserved, idl::Error::Denied => Refusal::Denied, idl::Error::Unsupported => Refusal::Unsupported }
}
fn pull_of(pull: idl::Pull) -> Pull { match pull { idl::Pull::None => Pull::None, idl::Pull::Up => Pull::Up, idl::Pull::Down => Pull::Down, idl::Pull::Unknown => Pull::Unknown } }
fn pull_to(pull: Pull) -> idl::Pull { match pull { Pull::None => idl::Pull::None, Pull::Up => idl::Pull::Up, Pull::Down => idl::Pull::Down, Pull::Unknown => idl::Pull::Unknown } }

impl Gpio for Service {
    fn controllers(&mut self) -> Result<Vec<Controller>, Refusal> {
        let list = idl::controllers(self.0).map_err(lost)?;
        Ok(list.as_slice().iter().map(|c| Controller {
            kind: String::from(match c.kind { idl::Kind::Bcm2711 => "bcm2711", idl::Kind::Pl061 => "pl061" }), pins: c.pins,
            soc: String::from(c.soc.as_str()), board: String::from(c.board.as_str()),
        }).collect())
    }
    fn pins(&mut self, controller: u8) -> Result<Vec<Pin>, Refusal> {
        let list = idl::pins(self.0, controller).map_err(lost)?.map_err(refused)?;
        Ok(list.as_slice().iter().map(|p| Pin { pin: p.pin, function: p.function, functions: p.functions, level: p.level, pull: pull_of(p.pull), reserved: p.reserved, position: p.position }).collect())
    }
    fn functions(&mut self, controller: u8, pin: u8) -> Result<Vec<String>, Refusal> {
        let list = idl::functions(self.0, controller, pin).map_err(lost)?.map_err(refused)?;
        Ok(list.as_slice().iter().map(|name| String::from(name.as_str())).collect())
    }
    fn set_function(&mut self, controller: u8, pin: u8, function: u8) -> Result<(), Refusal> { idl::set_function(self.0, controller, pin, function).map_err(lost)?.map_err(refused) }
    fn write(&mut self, controller: u8, pin: u8, high: bool) -> Result<(), Refusal> { idl::write(self.0, controller, pin, high).map_err(lost)?.map_err(refused) }
    fn set_pull(&mut self, controller: u8, pin: u8, pull: Pull) -> Result<(), Refusal> { idl::set_pull(self.0, controller, pin, pull_to(pull)).map_err(lost)?.map_err(refused) }
}

