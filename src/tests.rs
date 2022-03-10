use ordered_float::Float;
use assert_approx_eq::assert_approx_eq;

use super::*;
// use test::Bencher;

fn rounded(it:f32,places:i32) -> f32{
    let e = 10_f32.powi(places);
    (it * e).round() / e 
}

// #[test]
fn score_slot_test() {
    assert_eq!(15, score_slot(FIVES,[1,2,5,5,5]));
}

// #[test]
fn bench_test() {
    // let slots= array_vec!([u8;13] => 1,2,3,4,5,6,7,8,9,10,11,12,13);
    let game = GameState{   rolls_remaining: 0, 
                            sorted_open_slots: array_vec!([u8;13] => 6,8,12 ), 
                            sorted_dievals: [6,6,6,6,6], 
                            upper_bonus_deficit: 30, 
                            yahtzee_is_wild: false, };
    let app = AppState::new(&game);
    let _result = best_choice_ev(game,&app);
    // save_cache(&app);
}

#[test]
fn ev_of_yahtzee_in_1_roll() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 1, 
                            sorted_open_slots: array_vec!([u8;13] => 12 ), 
                            sorted_dievals: [1,2,3,4,5], 
                            upper_bonus_deficit: INIT_DEFICIT , 
                            yahtzee_is_wild: false, };
    let app = AppState::new(&game);
    let _result = best_choice_ev(game,&app);
    // eprintln!("{}, {}",rounded(_result.1, 4) , rounded( (6.0*1.0/6.0).powf(5.0) * 50.0, 4) );
    assert_approx_eq!( 
        _result.1 , 
        6.0 * (1.0/6.0).powi(5) * 50.0 
    );
}