use crate::target::SeccompRule;

use syscallz::{Action, Cmp, Comparator, Syscall};

pub(crate) fn allow_execve(
    argv0_ptr: *const libc::c_char,
    allow_target_execve: bool,
) -> SeccompRule {
    SeccompRule {
        action: Action::Allow,
        syscall: Syscall::execve,
        comparators: if allow_target_execve {
            vec![]
        } else {
            vec![Comparator::new(0, Cmp::Eq, argv0_ptr as u64, None)]
        },
    }
}
