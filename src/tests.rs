#![allow(dead_code)]

use std::io::BufWriter;

use assert_approx_eq::assert_approx_eq;

use super::*;
// use test::Bencher;

fn rounded(it:f32,places:i32) -> f32{
    let e = 10_f32.powi(places);
    (it * e).round() / e 
}

// #[test]
fn test_dievals_into(){
    let dv:DieVals = [2,2,5,5,5].into();
    assert_eq!(dv.get(5), 0);
    assert_eq!(dv.get(0), 2);
    eprintln!("{}",dv);
}


// #[test]
fn score_slot_test() {
    assert_eq!(0, score_slot(FULL_HOUSE,[1,2,5,5,5].into() ));
    assert_eq!(25, score_slot(FULL_HOUSE,[2,2,5,5,5].into()));
    assert_eq!(0, score_slot(YAHTZEE,[4,5,5,5,5].into()));
    assert_eq!(50, score_slot(YAHTZEE,[1,1,1,1,1].into()));
}

// #[test]
fn avg_ev_for_selection_test() {
    let game = GameState{   rolls_remaining: 1, 
                            sorted_open_slots: array_vec!([u16;13] => YAHTZEE ), 
                            sorted_dievals: [1,1,1,1,1].into(), 
                            upper_bonus_deficit: INIT_DEFICIT , yahtzee_is_wild: false, };
    let app = &mut AppState::new(&game);
    let sel = 31;
    let result = avg_ev_for_selection(game, app, sel) ;
    assert_eq!(result,50.0);
}
 
// #[test]
fn ev_of_yahtzee_in_1_roll() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 1, 
                            sorted_open_slots: array_vec!([u16;13] => YAHTZEE ), 
                            sorted_dievals: UNROLLED_DIEVALS, 
                            upper_bonus_deficit: INIT_DEFICIT , yahtzee_is_wild: false, };
    let app = &mut AppState::new(&game);
    let _result = best_choice_ev(game, app);
    let in_1_odds = 6.0/7776.0; 
    assert_approx_eq!( _result.ev , in_1_odds * 50.0 );
}

// #[test]
fn ev_of_yahtzee_in_2_rolls() {
    let game = GameState{   rolls_remaining: 2, 
                            sorted_open_slots: array_vec!([u16;13] => YAHTZEE ), 
                            sorted_dievals: UNROLLED_DIEVALS, 
                            upper_bonus_deficit: INIT_DEFICIT , yahtzee_is_wild: false, };
    let app = &mut AppState::new(&game);
    let _result = best_choice_ev(game, app);
    let in_2 = 0.01263; //https://www.datagenetics.com/blog/january42012/    
    // let in_1 = 6.0/7776.0; //0.00077; 
    assert_approx_eq!( rounded( _result.ev ,3), rounded( (in_2)*50.0, 3) );
}

// #[test]
fn ev_of_yahtzee_in_3_rolls() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 3, 
                            sorted_open_slots: array_vec!([u16;13] => YAHTZEE ), 
                            sorted_dievals: UNROLLED_DIEVALS, 
                            upper_bonus_deficit: INIT_DEFICIT , yahtzee_is_wild: false, };
    let app = &mut AppState::new(&game);
    let _result = best_choice_ev(game, app);
    assert_approx_eq!( rounded( _result.ev, 2), rounded( 0.04603 * 50.0, 2) );
}

// #[test] 
fn ev_of_smstraight_in_1() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 1, 
                            sorted_open_slots: array_vec!([u16;13] => SM_STRAIGHT ), 
                            sorted_dievals: [0,0,0,0,0].into(), 
                            upper_bonus_deficit: INIT_DEFICIT , yahtzee_is_wild: false, };
    let app = &mut AppState::new(&game);
    let result = best_choice_ev(game, app);
    assert_eq!( rounded( result.ev / 30.0, 2), rounded( 0.1235 + 0.0309 , 2) );
}

// #[test]
fn ev_of_lgstraight_in_1() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 1, 
                            sorted_open_slots: array_vec!([u16;13] => LG_STRAIGHT ), 
                            sorted_dievals: UNROLLED_DIEVALS, 
                            upper_bonus_deficit: INIT_DEFICIT , yahtzee_is_wild: false, };
    let app = &mut AppState::new(&game);
    let result = best_choice_ev(game, app);
    assert_eq!( rounded( result.ev  / 40.0, 4), rounded( 0.0309, 4) );
}

// #[test] 
fn ev_of_4ofakind_in_1() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 1, 
                            sorted_open_slots: array_vec!([u16;13] => FOUR_OF_A_KIND), 
                            sorted_dievals: UNROLLED_DIEVALS, 
                            upper_bonus_deficit: INIT_DEFICIT , yahtzee_is_wild: false, };
    let app = &mut AppState::new(&game);
    let result = best_choice_ev(game, app);
    assert_eq!( 
        rounded( result.ev  / 17.5, 3), // we divide EV by average dice-total to get odds
        rounded( 0.0193+0.00077, 3) //our 3 of a kind includes 4 of a kind & yahtzee
    );
}

// #[test] 
fn ev_of_3ofakind_in_1() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 1, 
                            sorted_open_slots: array_vec!([u16;13] => THREE_OF_A_KIND), 
                            sorted_dievals: UNROLLED_DIEVALS, 
                            upper_bonus_deficit: INIT_DEFICIT , yahtzee_is_wild: false, };
    let app = &mut AppState::new(&game);
    let result = best_choice_ev(game, app); 
    assert_eq!( 
        rounded( result.ev  / 17.5, 3), // we divide EV by average dice-total to get odds
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

// #[test]
fn test_permutations() {

    let a = SlotPermutations::new( array_vec!([u16;13] => 0,1,2) );
    for perm in a { 
        println!("{}", perm); 
    }; 
}

#[test]
fn bench_test() {
    // let slots= array_vec!([u16;13] => 1,2,3,4,5,6,7,8,9,10,11,12,13);
    let game = GameState{   rolls_remaining: 0, 
                            sorted_open_slots: array_vec!([u16;13] => SIXES,FOUR_OF_A_KIND,YAHTZEE ), 
                            sorted_dievals: UNROLLED_DIEVALS, 
                            upper_bonus_deficit: 30, 
                            yahtzee_is_wild: false, };
    let app = &mut AppState::new(&game);
    let result = best_choice_ev(game, app);
    assert_eq!(rounded(result.ev,2),  28.92);
    // save_cache(&app);
}

// #[test]
fn print_misc() {
    // eprint!("{:?}", selection_ranges() );
    // eprint!("{:?}", all_selection_outcomes() );
    eprintln!("{:?}", selection_ranges() );
    // eprint!("{:?}", five_dice_combinations() );
    // eprint!("{:?}", die_index_combos() );
}