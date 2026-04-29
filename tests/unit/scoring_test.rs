//! Unit tests for scoring module

use code_golf::scoring;

#[test]
fn test_compute_par_score_easy() {
    let user_bytes = 10;
    let par_bytes = 20;
    
    let score = scoring::compute_par_score(user_bytes, par_bytes);
    
    // Score should be 100 (perfect)
    assert_eq!(score, 100);
}

#[test]
fn test_compute_par_score_medium() {
    let user_bytes = 15;
    let par_bytes = 20;
    
    let score = scoring::compute_par_score(user_bytes, par_bytes);
    
    // Score should be 50 (halfway)
    assert_eq!(score, 50);
}

#[test]
fn test_compute_par_score_hard() {
    let user_bytes = 19;
    let par_bytes = 20;
    
    let score = scoring::compute_par_score(user_bytes, par_bytes);
    
    // Score should be 10
    assert_eq!(score, 10);
}

#[test]
fn test_compute_par_score_better_than_par() {
    let user_bytes = 5;
    let par_bytes = 20;
    
    let score = scoring::compute_par_score(user_bytes, par_bytes);
    
    // Score should be capped at 100
    assert_eq!(score, 100);
}

#[test]
fn test_compute_par_score_worse_than_par() {
    let user_bytes = 30;
    let par_bytes = 20;
    
    let score = scoring::compute_par_score(user_bytes, par_bytes);
    
    // Score should be negative
    assert!(score < 0);
}

#[test]
fn test_compute_par_score_zero_bytes() {
    let user_bytes = 0;
    let par_bytes = 20;
    
    let score = scoring::compute_par_score(user_bytes, par_bytes);
    
    // Perfect score for zero bytes
    assert_eq!(score, 100);
}

#[test]
fn test_compute_par_score_equal_bytes() {
    let user_bytes = 25;
    let par_bytes = 25;
    
    let score = scoring::compute_par_score(user_bytes, par_bytes);
    
    // Score should be 0 when equal
    assert_eq!(score, 0);
}

#[test]
fn test_par_score_name_100() {
    assert_eq!(scoring::par_score_name(100), "OG");
}

#[test]
fn test_par_score_name_90() {
    assert_eq!(scoring::par_score_name(90), "MVP");
}

#[test]
fn test_par_score_name_80() {
    assert_eq!(scoring::par_score_name(80), "GODLIKE");
}

#[test]
fn test_par_score_name_70() {
    assert_eq!(scoring::par_score_name(70), "LEGENDARY");
}

#[test]
fn test_par_score_name_60() {
    assert_eq!(scoring::par_score_name(60), "ELITE");
}

#[test]
fn test_par_score_name_50() {
    assert_eq!(scoring::par_score_name(50), "ADVANCED");
}

#[test]
fn test_par_score_name_0() {
    assert_eq!(scoring::par_score_name(0), "BEGINNER");
}

#[test]
fn test_par_score_name_negative() {
    assert_eq!(scoring::par_score_name(-50), "NOOB");
}

#[test]
fn test_par_score_name_below_noob() {
    assert_eq!(scoring::par_score_name(-100), "BOT");
}

#[test]
fn test_par_score_name_boundary() {
    // Test boundary between scores
    assert_eq!(scoring::par_score_name(99), "MVP");
    assert_eq!(scoring::par_score_name(91), "MVP");
    assert_eq!(scoring::par_score_name(90), "MVP");
    assert_eq!(scoring::par_score_name(89), "GODLIKE");
}

#[test]
#[should_panic(expected = "Par score must be between -100 and 100")]
fn test_par_score_name_out_of_range_high() {
    scoring::par_score_name(101);
}

#[test]
#[should_panic(expected = "Par score must be between -100 and 100")]
fn test_par_score_name_out_of_range_low() {
    scoring::par_score_name(-101);
}
