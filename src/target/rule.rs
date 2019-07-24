use syscallz::{Action, Comparator, Context, Syscall};

#[derive(Debug, Clone)]
pub struct SeccompRule {
    pub action: Action,
    pub syscall: Syscall,
    pub comparators: Vec<Comparator>,
}

#[derive(Debug, Clone)]
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
    pub(super) fn apply_seccomp(&self, extra_rules: &[SeccompRule]) -> std::io::Result<()> {
        if self.default_action.is_none() && extra_rules.is_empty() {
            return Ok(());
        }

        let default = self.default_action.unwrap_or(Action::Allow);
        let mut ctx = Context::init_with_action(default)?;

        for rule in self.seccomp_rules.iter().chain(extra_rules) {
            if rule.comparators.is_empty() {
                ctx.set_action_for_syscall(rule.action, rule.syscall)?;
            } else {
                for comp in &rule.comparators {
                    let res = ctx.set_rule_for_syscall(
                        rule.action,
                        rule.syscall,
                        std::slice::from_ref(comp),
                    );
                    dbg!((comp, &res));
                    res?;
                }
            }
        }

        ctx.load()?;
        Ok(())
    }
}
