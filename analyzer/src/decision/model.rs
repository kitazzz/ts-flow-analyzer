use std::collections::HashMap;

pub type DecisionValue = &'static str; // "T", "F", "*"

#[derive(Debug, Clone)]
pub struct PathState {
    pub assignments: HashMap<usize, &'static str>, // index -> "T" or "F"
}

impl PathState {
    pub fn new() -> Self {
        PathState {
            assignments: HashMap::new(),
        }
    }

    pub fn with_assignment(&self, index: usize, value: &'static str) -> Self {
        let mut clone = self.clone();
        clone.assignments.insert(index, value);
        clone
    }
}

#[derive(Debug, Clone)]
pub struct PathOutcome {
    pub assignments: HashMap<usize, &'static str>,
    pub outcome: String,
    pub outcome_kind: String, // "return" or "throw"
    pub line: u32,
}

#[derive(Debug, Clone)]
pub struct WalkResult {
    pub terminated: Vec<PathOutcome>,
    pub continued: Vec<PathState>,
}
