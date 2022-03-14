use std::io::BufWriter;

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
fn ev_of_yahtzee_in_1_roll() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 1, 
                            sorted_open_slots: array_vec!([u8;13] => YAHTZEE ), 
                            sorted_dievals: UNROLLED_DIEVALS, 
                            upper_bonus_deficit: INIT_DEFICIT , yahtzee_is_wild: false, };
    let app = AppState::new(&game);
    let _result = best_choice_ev(game,&app);
    // eprintln!("{}, {}",rounded(_result.1, 4) , rounded( (6.0*1.0/6.0).powf(5.0) * 50.0, 4) );
    assert_approx_eq!( _result.1 , 6.0 * (1.0/6.0).powi(5) * 50.0 );
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
    assert_eq!( rounded( _result.1, 1), rounded( 0.0461 * 50.0, 1) );
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
    assert_eq!( rounded( result.1, 1), rounded( 0.0386 * 25.0, 1) );
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
    assert_eq!( rounded( result.1 / 30.0, 2), rounded( 0.1235 + 0.0309 , 2) );
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
    assert_eq!( rounded( result.1  / 40.0, 4), rounded( 0.0309, 4) );
}

// #[test] 
fn ev_of_4ofakind_in_1() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 1, 
                            sorted_open_slots: array_vec!([u8;13] => FOUR_OF_A_KIND), 
                            sorted_dievals: UNROLLED_DIEVALS, 
                            upper_bonus_deficit: INIT_DEFICIT , yahtzee_is_wild: false, };
    let app = AppState::new(&game);
    let result = best_choice_ev(game,&app);
    assert_eq!( 
        rounded( result.1  / 17.5, 3), // we divide EV by average dice-total to get odds
        rounded( 0.0193+0.00077, 3) //our 3 of a kind includes 4 of a kind & yahtzee
    );
}

// #[test] 
fn ev_of_3ofakind_in_1() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 1, 
                            sorted_open_slots: array_vec!([u8;13] => THREE_OF_A_KIND), 
                            sorted_dievals: UNROLLED_DIEVALS, 
                            upper_bonus_deficit: INIT_DEFICIT , yahtzee_is_wild: false, };
    let app = AppState::new(&game);
    let result = best_choice_ev(game,&app); 
    assert_eq!( 
        rounded( result.1  / 17.5, 3), // we divide EV by average dice-total to get odds
        rounded( 0.1929+0.0193+0.0007, 3) //our 3 of a kind includes 4 of a kind & yahtzee
    );
}


// #[test]
fn make_permutations(){
    for n in 1_u8..=13_u8 {
        let filename = "perms".to_string() + &n.to_string();
        let mut f = BufWriter::with_capacity(1_000_000_000,File::create(filename).unwrap());
        for perms in &(1_u8..=n).into_iter().permutations(n as usize).chunks(1024) {
            bincode::serialize_into(&mut f,&perms.collect_vec()).unwrap();
        }
    }
}
// Output: 
//             17 Mar 12 10:23 perms1
//       65346752 Mar 12 10:23 perms10
//      758731056 Mar 12 10:23 perms11
//     9583774200 Mar 12 10:23 perms12
//   130816085400 Mar 12 10:32 perms13
//             28 Mar 12 10:23 perms2
//             74 Mar 12 10:23 perms3
//            296 Mar 12 10:23 perms4
//           1568 Mar 12 10:23 perms5
//          10088 Mar 12 10:23 perms6
//          75640 Mar 12 10:23 perms7
//         645440 Mar 12 10:23 perms8
//        6171800 Mar 12 10:23 perms9

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

