use crate::target::{SeccompRule, TargetRule};

use syscallz::{Action, Cmp, Comparator, Syscall};

pub fn c_cpp_rules() -> TargetRule {
    const SYSCALLS: [Syscall; 16] = [
        Syscall::mprotect,
        Syscall::mmap,
        Syscall::access,
        Syscall::read,
        Syscall::close,
        Syscall::stat,
        Syscall::fstat,
        Syscall::munmap,
        Syscall::brk,
        Syscall::arch_prctl,
        Syscall::write,
        Syscall::lseek,
        Syscall::uname,
        Syscall::readlink,
        Syscall::exit_group,
        Syscall::sysinfo,
    ];

    let mut rule = TargetRule::from_default_action(Action::Kill);
    for &syscall in &SYSCALLS {
        rule.add_action(Action::Allow, syscall);
    }

    // handle [open, openat]
    rule.add_rule(SeccompRule {
        action: Action::Allow,
        syscall: Syscall::open,
        comparators: vec![Comparator::new(
            1,
            Cmp::MaskedEq,
            0x11,
            libc::O_RDONLY as u64,
        )],
    });

    rule.add_rule(SeccompRule {
        action: Action::Allow,
        syscall: Syscall::openat,
        comparators: vec![Comparator::new(
            2,
            Cmp::MaskedEq,
            0x11,
            libc::O_RDONLY as u64,
        )],
    });

    rule
}
