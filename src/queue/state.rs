//! Queue state management

/// Queue entry status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueStatus {
    Pending,
    Searching,
    Matched,
    NeedsReview,
    NeedsManual,
    Downloading,
    Downloaded,
    Installing,
    Completed,
    Failed,
    Skipped,
}

impl QueueStatus {
    pub fn from_str(s: &str) -> Self {
        match s {
            "pending" => QueueStatus::Pending,
            "searching" => QueueStatus::Searching,
            "matched" => QueueStatus::Matched,
            "needs_review" => QueueStatus::NeedsReview,
            "needs_manual" => QueueStatus::NeedsManual,
            "downloading" => QueueStatus::Downloading,
            "downloaded" => QueueStatus::Downloaded,
            "installing" => QueueStatus::Installing,
            "completed" => QueueStatus::Completed,
            "failed" => QueueStatus::Failed,
            "skipped" => QueueStatus::Skipped,
            _ => QueueStatus::Pending,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            QueueStatus::Pending => "pending",
            QueueStatus::Searching => "searching",
            QueueStatus::Matched => "matched",
            QueueStatus::NeedsReview => "needs_review",
            QueueStatus::NeedsManual => "needs_manual",
            QueueStatus::Downloading => "downloading",
            QueueStatus::Downloaded => "downloaded",
            QueueStatus::Installing => "installing",
            QueueStatus::Completed => "completed",
            QueueStatus::Failed => "failed",
            QueueStatus::Skipped => "skipped",
        }
        .to_string()
    }

    pub fn is_final(&self) -> bool {
        matches!(
            self,
            QueueStatus::Completed | QueueStatus::Failed | QueueStatus::Skipped
        )
    }

    pub fn is_actionable(&self) -> bool {
        matches!(self, QueueStatus::Matched | QueueStatus::Downloaded)
    }
}

/// Overall queue state
#[derive(Debug, Clone)]
pub struct QueueState {
    pub total: usize,
    pub pending: usize,
    pub downloading: usize,
    pub completed: usize,
    pub failed: usize,
    pub needs_review: usize,
}

impl QueueState {
    pub fn new() -> Self {
        Self {
            total: 0,
            pending: 0,
            downloading: 0,
            completed: 0,
            failed: 0,
            needs_review: 0,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.pending == 0 && self.downloading == 0
    }

    pub fn progress(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.completed as f32 / self.total as f32
        }
    }
}
