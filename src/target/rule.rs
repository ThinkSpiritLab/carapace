use std::io::Result;
use syscallz::{Action, Comparator, Context, Syscall};

#[derive(Clone)]
pub struct SeccompRule {
    pub action: Action,
    pub syscall: Syscall,
    pub comparators: Vec<Comparator>,
}

#[derive(Clone)]
pub struct TargetRule {
    pub default_action: Option<Action>,
    pub seccomp_rules: Vec<SeccompRule>,
}

impl TargetRule {
    pub fn new() -> Self {
        Self {
            default_action: None,
            seccomp_rules: vec![],
        }
    }
}

impl TargetRule {
    // syscallz::Error is not std::io::Error
    fn wrapped_apply(&self) -> syscallz::Result<()> {
        let defalut = match self.default_action {
            None => return Ok(()),
            Some(default) => default,
        };

        let mut ctx = Context::init_with_action(defalut)?;

        for rule in &self.seccomp_rules {
            ctx.set_rule_for_syscall(rule.action, rule.syscall, &rule.comparators)?;
        }

        ctx.load()
    }

    pub(super) fn apply_seccomp(&self) -> Result<()> {
        self.wrapped_apply()
            .map_err(|_| std::io::Error::last_os_error())
    }
}
