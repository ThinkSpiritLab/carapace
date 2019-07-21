use rlimit::RLimit;

#[derive(Debug, Clone)]
pub struct TargetLimit {
    pub max_real_time: Option<u64>,   // in microseconds
    pub max_cpu_time: Option<u64>,    // in seconds
    pub max_memory: Option<u64>,      // in bytes
    pub max_output_size: Option<u64>, // in bytes
    pub max_process_number: Option<u64>,
    pub max_stack_size: Option<u64>, // in bytes
}

impl TargetLimit {
    pub(crate) fn apply_rlimit(&self) -> std::io::Result<()> {
        if let Some(n) = self.max_cpu_time {
            RLimit::CPU.set(n, n)?;
        }

        if let Some(n) = self.max_memory {
            RLimit::AS.set(n, n)?;
        }

        if let Some(n) = self.max_output_size {
            RLimit::FSIZE.set(n, n)?;
        }

        if let Some(n) = self.max_process_number {
            RLimit::NPROC.set(n, n)?;
        }

        if let Some(n) = self.max_stack_size {
            RLimit::STACK.set(n, n)?;
        }
        Ok(())
    }
}
