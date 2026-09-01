use anyhow::Result;
use std::time::{Duration, Instant};

/// One absolute wall-clock deadline shared by every phase of a structured
/// file scan. Clones retain the same deadline rather than restarting a phase
/// budget.
#[derive(Debug, Clone, Copy)]
pub struct SamplingDeadline {
    deadline: Option<Instant>,
}

impl SamplingDeadline {
    pub fn unlimited() -> Self {
        Self { deadline: None }
    }

    pub fn after(max_wall: Duration) -> Self {
        if max_wall == Duration::MAX {
            return Self::unlimited();
        }
        Self {
            deadline: Instant::now().checked_add(max_wall),
        }
    }

    pub fn check(&self, phase: &str) -> Result<()> {
        if self.is_expired() {
            anyhow::bail!("structured sampling deadline expired while {phase}");
        }
        Ok(())
    }

    pub fn is_expired(&self) -> bool {
        self.deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloned_deadlines_do_not_restart_the_budget() {
        let deadline = SamplingDeadline::after(Duration::ZERO);
        let clone = deadline;
        assert!(deadline.is_expired());
        assert!(clone.check("testing a shared deadline").is_err());
    }

    #[test]
    fn unlimited_deadline_never_expires() {
        assert!(!SamplingDeadline::unlimited().is_expired());
    }

    #[test]
    fn maximum_duration_is_explicitly_unlimited() {
        let deadline = SamplingDeadline::after(Duration::MAX);
        assert!(deadline.deadline.is_none());
        assert!(!deadline.is_expired());
    }
}
