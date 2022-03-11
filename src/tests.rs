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

#[test]
fn bench_test() {
    // let slots= array_vec!([u8;13] => 1,2,3,4,5,6,7,8,9,10,11,12,13);
    let game = GameState{   rolls_remaining: 0, 
                            sorted_open_slots: array_vec!([u8;13] => SIXES,FOUR_OF_A_KIND,YAHTZEE ), 
                            sorted_dievals: UNROLLED_DIEVALS, 
                            upper_bonus_deficit: 30, 
                            yahtzee_is_wild: false, };
    let app = AppState::new(&game);
    let _result = best_choice_ev(game,&app);
    // save_cache(&app);
}

// #[test]
fn ev_of_yahtzee_in_1_roll() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 1, 
                            sorted_open_slots: array_vec!([u8;13] => YAHTZEE ), 
                            sorted_dievals: UNROLLED_DIEVALS, 
                            upper_bonus_deficit: INIT_DEFICIT , yahtzee_is_wild: false, };
    let app = AppState::new(&game);
    let _result = best_choice_ev(game,&app);
    // eprintln!("{}, {}",rounded(_result.1, 4) , rounded( (6.0*1.0/6.0).powf(5.0) * 50.0, 4) );
    assert_approx_eq!( 
        _result.1 , 
        6.0 * (1.0/6.0).powi(5) * 50.0 
    );
}

// #[test]
fn ev_of_yahtzee_in_3_rolls() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 3, 
                            sorted_open_slots: array_vec!([u8;13] => YAHTZEE ), 
                            sorted_dievals: UNROLLED_DIEVALS, 
                            upper_bonus_deficit: INIT_DEFICIT , yahtzee_is_wild: false, };
    let app = AppState::new(&game);
    let _result = best_choice_ev(game,&app);
    assert_eq!( 
        rounded( _result.1, 1), 
        rounded( 0.0461 * 50.0, 1) 
    );
}


// #[test]
fn ev_of_fullhouse_in_1() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 1, 
                            sorted_open_slots: array_vec!([u8;13] => FULL_HOUSE ), 
                            sorted_dievals: UNROLLED_DIEVALS, 
                            upper_bonus_deficit: INIT_DEFICIT , yahtzee_is_wild: false, };
    let app = AppState::new(&game);
    let result = best_choice_ev(game,&app);
    assert_eq!( 
        rounded( result.1, 1), 
        rounded( 0.0386 * 25.0, 1) 
    );
}

// #[test] 
fn ev_of_smstraight_in_1() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 1, 
                            sorted_open_slots: array_vec!([u8;13] => SM_STRAIGHT ), 
                            sorted_dievals: [0,0,0,0,0], 
                            upper_bonus_deficit: INIT_DEFICIT , yahtzee_is_wild: false, };
    let app = AppState::new(&game);
    let result = best_choice_ev(game,&app);
    assert_eq!( 
        rounded( result.1 / 30.0, 2), rounded( 0.1235 + 0.0309 , 2) 
    );
}

// #[test]
fn ev_of_lgstraight_in_1() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 1, 
                            sorted_open_slots: array_vec!([u8;13] => LG_STRAIGHT ), 
                            sorted_dievals: UNROLLED_DIEVALS, 
                            upper_bonus_deficit: INIT_DEFICIT , yahtzee_is_wild: false, };
    let app = AppState::new(&game);
    let result = best_choice_ev(game,&app);
    assert_eq!( 
        rounded( result.1  / 40.0, 4), 
        rounded( 0.0309, 4) 
    );
}

// #[test]
// fn make_permutations(){
//     for n in 1_u8..=13_u8 {
//         let perms = (1_u8..=n).into_iter().permutations(n as usize).collect_vec();
//         eprintln!("{}",n);
//         let filename = "perms".to_string() + &n.to_string();
//         let mut f = &File::create(filename).unwrap();
//         let bytes = bincode::serialize(&perms).unwrap();
//         f.write_all(&bytes).unwrap();
//     }
// }