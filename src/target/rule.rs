use syscallz::{Action, Comparator, Context, Syscall};

#[derive(Debug, Clone)]
pub struct SeccompRule {
    pub action: Action,
    pub syscall: Syscall,
    pub comparators: Vec<Comparator>,
}

impl SeccompRule {
    #[inline]
    pub fn without_comparators(action: Action, syscall: Syscall) -> Self {
        Self {
            action,
            syscall,
            comparators: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct TargetRule {
    pub default_action: Action,
    pub seccomp_rules: Vec<SeccompRule>,
}

impl TargetRule {
    pub fn new() -> Self {
        Self {
            default_action: Action::Allow,
            seccomp_rules: vec![],
        }
    }

    pub fn from_default_action(action: Action) -> Self {
        Self {
            default_action: action,
            seccomp_rules: vec![],
        }
    }

    pub fn add_action(&mut self, action: Action, syscall: Syscall) {
        self.seccomp_rules
            .push(SeccompRule::without_comparators(action, syscall));
    }

    pub fn add_rule(&mut self, rule: SeccompRule) {
        self.seccomp_rules.push(rule);
    }
}

impl TargetRule {
    pub(super) fn apply_seccomp(&self, extra_rules: &[SeccompRule]) -> std::io::Result<()> {
        if let Action::Allow = self.default_action {
            if self.seccomp_rules.is_empty() && extra_rules.is_empty() {
                return Ok(());
            }
        }

        let mut ctx = Context::init_with_action(self.default_action)?;

        for rule in self.seccomp_rules.iter().chain(extra_rules) {
            if rule.comparators.is_empty() {
                ctx.set_action_for_syscall(rule.action, rule.syscall)?;
            } else {
                for comp in &rule.comparators {
                    ctx.set_rule_for_syscall(
                        rule.action,
                        rule.syscall,
                        std::slice::from_ref(comp),
                    )?;
                }
            }
        }

        ctx.load()?;
        Ok(())
    }
}
