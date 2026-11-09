// A call's buffer, as the IDL's generated clients make them (000-KRN-0226): `Pages` overwritten with zeros before they
// are freed, so that what a call carried, a secret among it, is not left in frames the kernel takes back. Shared with
// the IDL's host tests (tests/idl_host.rs), whose `Pages` stands in for the kernel's.
use super::Pages;

pub struct Wiped(pub Pages);
impl core::ops::Deref for Wiped { type Target = Pages; fn deref(&self) -> &Pages { &self.0 } }
impl core::ops::DerefMut for Wiped { fn deref_mut(&mut self) -> &mut Pages { &mut self.0 } }
impl Drop for Wiped {
    // Volatile stores, which the compiler keeps although the pages are freed next; a page is a whole number of words.
    fn drop(&mut self) { for word in self.0.as_mut_slice().chunks_exact_mut(8) { unsafe { core::ptr::write_volatile(word.as_mut_ptr() as *mut u64, 0) } } }
}
