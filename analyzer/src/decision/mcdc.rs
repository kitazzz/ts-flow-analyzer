use crate::model::{DecisionPoint, McdcCase, TruthRow};

pub fn build_mcdc_cases(decisions: &[DecisionPoint], rows: &[TruthRow]) -> Vec<McdcCase> {
    let mut cases = Vec::new();
    for decision in decisions {
        if let Some(pair) = find_mcdc_pair(rows, decision.index) {
            cases.push(McdcCase {
                predicate: decision.predicate.clone(),
                index: decision.index,
                row_false: pair.0,
                row_true: pair.1,
            });
        }
    }
    cases
}

fn find_mcdc_pair(rows: &[TruthRow], index: usize) -> Option<(TruthRow, TruthRow)> {
    let mut best: Option<(TruthRow, TruthRow, i32)> = None;

    // O6d: exclude rows whose terminal block was marked dead by the CFG builder
    let rows: Vec<&TruthRow> = rows
        .iter()
        .filter(|r| r.terminal_reachable != Some(false))
        .collect();

    for row_false in &rows {
        if row_false.values[index] != "F" {
            continue;
        }
        for row_true in &rows {
            if row_true.values[index] != "T" {
                continue;
            }
            if row_false.outcome_label == row_true.outcome_label {
                continue;
            }
            let score = compare_rows_for_mcdc(row_false, row_true, index);
            if score < 0 {
                continue;
            }
            match &best {
                None => {
                    best = Some(((*row_false).clone(), (*row_true).clone(), score));
                }
                Some((_, _, best_score)) if score > *best_score => {
                    best = Some(((*row_false).clone(), (*row_true).clone(), score));
                }
                _ => {}
            }
        }
    }

    best.map(|(f, t, _)| (f, t))
}

fn compare_rows_for_mcdc(a: &TruthRow, b: &TruthRow, focus: usize) -> i32 {
    let mut score: i32 = 0;
    let len = a.values.len().min(b.values.len());

    for i in 0..len {
        if i == focus {
            continue;
        }
        let left = &a.values[i];
        let right = &b.values[i];
        if left == right {
            if left != "*" {
                score += 2;
            }
        } else if left == "*" || right == "*" {
            score += 1;
        } else {
            return -1;
        }
    }

    if a.outcome_kind == "happy" || b.outcome_kind == "happy" {
        score += 10;
    }
    if a.outcome_kind == "happy" {
        score += 2;
    }
    if b.outcome_kind == "happy" {
        score += 2;
    }

    score
}
