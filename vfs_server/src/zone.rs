// What a handle may change (its zone), and the zone of each name below a directory by the client's badge; kept apart
// from the server for the host tests (tests/vfs_zone_host.rs).
use crate::fat::same_name;
use super::{BADGE_KEYSTORE, BADGE_NETPOLICY, BADGE_UPDATE}; // libmind's (mind::fs)

/// What a handle may change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone { ReadOnly, Writable, BootRoot, BootReadOnly, System, Hidden, Slots, Record }

/// The private directories of the boot disk's `system/`, each with the only badge that may open, read or write it.
pub const PRIVATE: [(&str, u16); 2] = [("keystore", BADGE_KEYSTORE), ("netpolicy", BADGE_NETPOLICY)];

/// A boot record's size: it is written whole, in place (docs/update/slots.md).
pub const RECORD: u32 = 512;

impl Zone {
    /// The zone of `name` below a directory of this zone, for a client with `badge`. `inactive` is the slot the system
    /// did not boot from ("A" or "B"), None when it booted from the volume's root. Below the boot root `data` is the
    /// user's to write and `system` holds private directories; below `system` everything is hidden but the client's
    /// own. The update badge sees `MIND` as the slots' directory (351-UPD-0008): the inactive slot is writable there,
    /// the boot records only in place, and the running slot is read-only like everything else.
    pub fn below(self, name: &str, badge: u16, inactive: Option<&str>) -> Zone {
        match self {
            Zone::BootRoot if same_name(name, "data") => Zone::Writable,
            Zone::BootRoot | Zone::BootReadOnly if same_name(name, "system") => Zone::System,
            Zone::BootReadOnly if badge == BADGE_UPDATE && inactive.is_some() && same_name(name, "MIND") => Zone::Slots,
            Zone::BootRoot | Zone::BootReadOnly => Zone::ReadOnly,
            Zone::System => match PRIVATE.iter().find(|(dir, _)| same_name(name, dir)) {
                Some(&(_, owner)) if owner == badge => Zone::Writable,
                _ => Zone::Hidden,
            },
            Zone::Slots if inactive.is_some_and(|slot| same_name(name, slot)) => Zone::Writable,
            Zone::Slots if same_name(name, "BOOT0") || same_name(name, "BOOT1") => Zone::Record,
            Zone::Slots | Zone::Record => Zone::ReadOnly,
            zone => zone,
        }
    }
}

