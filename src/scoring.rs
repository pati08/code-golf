/// Compute a golf-style par score from player bytes relative to par bytes.
///
/// Uses percentage bands so the score feels natural regardless of problem size:
/// ≤ 50%  → -3 (Albatross)
/// ≤ 75%  → -2 (Eagle)
/// ≤ 90%  → -1 (Birdie)
/// ≤ 110% →  0 (Par)
/// ≤ 140% → +1 (Bogey)
/// ≤ 170% → +2 (Double Bogey)
///  > 170% → +3 (Triple Bogey)
pub fn compute_par_score(player_bytes: i64, par_bytes: i64) -> i32 {
    if par_bytes <= 0 {
        return 0;
    }
    let ratio = player_bytes as f64 / par_bytes as f64;
    if ratio <= 0.50 {
        -3
    } else if ratio <= 0.75 {
        -2
    } else if ratio <= 0.90 {
        -1
    } else if ratio <= 1.10 {
        0
    } else if ratio <= 1.40 {
        1
    } else if ratio <= 1.70 {
        2
    } else {
        3
    }
}

pub fn par_score_name(score: i32) -> &'static str {
    match score {
        -3 => "Albatross",
        -2 => "Eagle",
        -1 => "Birdie",
        0 => "Par",
        1 => "Bogey",
        2 => "Double Bogey",
        _ => "Triple Bogey",
    }
}
