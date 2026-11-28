//! The Node stand-in's constants (node/src/layout.rs) against common/abi.rs as a 64-bit program sees it
//! (215-PRT-0016): an ABI step that moves them fails here, not in the stand-in.
#[allow(dead_code)]
#[path = "../../common/abi.rs"]
mod abi;
#[allow(dead_code)]
#[path = "../node/src/layout.rs"]
mod layout;
use core::mem::offset_of;

#[test]
fn the_stand_in_s_layout_is_the_abi_s() {
    assert_eq!(offset_of!(abi::BootInfo, abi_version), layout::BOOT_INFO_ABI_VERSION);
    assert_eq!(abi::ABI_VERSION, layout::ABI_VERSION);
    assert_eq!(offset_of!(abi::SyscallMailbox, syscall_num), layout::MAILBOX_NUMBER);
    assert_eq!(offset_of!(abi::SyscallMailbox, arg1), layout::MAILBOX_ARG1);
    assert_eq!(offset_of!(abi::SyscallMailbox, arg2), layout::MAILBOX_ARG2);
    assert_eq!(offset_of!(abi::SyscallMailbox, result), layout::MAILBOX_RESULT);
    let calls = [(abi::SYSCALL_LOG, layout::SYSCALL_LOG), (abi::SYSCALL_UPTIME, layout::SYSCALL_UPTIME), (abi::SYSCALL_EXIT, layout::SYSCALL_EXIT),
        (abi::SYSCALL_ALLOC, layout::SYSCALL_ALLOC), (abi::SYSCALL_FREE, layout::SYSCALL_FREE)];
    for (number, stand_in) in calls { assert_eq!(number as u64, stand_in); }
    assert_eq!(abi::ERR_INVALID as u64, layout::ERR_INVALID);
}
