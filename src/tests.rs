#![allow(dead_code)]

use std::io::BufWriter;

use assert_approx_eq::assert_approx_eq;

use super::*;

fn print_state_choice(state: &GameState, choice_ev:ChoiceEV){
    if state.rolls_remaining==0 {
        println!("S\t{: >6.2?}\t{:_^5}\t{:2?}\t{}\t{:2?}\t{}\t{: <29}",
            choice_ev.ev, choice_ev.choice, state.rolls_remaining, state.sorted_dievals, state.upper_total, 
            if state.yahtzee_bonus_avail {"Y"}else{""}, state.sorted_open_slots.to_string()); 
    } else {
        println!("D\t{: >6.2?}\t{:05b}\t{:2?}\t{}\t{:2?}\t{}\t{: <29}",
            choice_ev.ev, choice_ev.choice, state.rolls_remaining, state.sorted_dievals, state.upper_total, 
            if state.yahtzee_bonus_avail {"Y"}else{""}, state.sorted_open_slots.to_string()); 
    };
}

fn best_choice_ev(game:GameState, app: &mut AppState) -> ChoiceEV{
    debug_assert!(app.ev_cache.is_empty());
    build_cache(game, app);
    *app.ev_cache.get(&game).unwrap()
}

fn rounded(it:f32,places:i32) -> f32{
    let e = 10_f32.powi(places);
    (it * e).round() / e 
}

// #[test]
fn test_dievals_into(){
    let dv:DieVals = [2,2,5,5,5].into();
    assert_eq!(dv.get(4), 5);
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
fn ev_of_yahtzee_in_1_roll() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 1, 
                            sorted_open_slots: [YAHTZEE].into(), 
                            sorted_dievals: [1,2,3,4,5].into(),
                            ..default() };
    let app = &mut AppState::new(&game);
    let _result = best_choice_ev(game, app);
    let in_1_odds = 6.0/7776.0; 
    assert_approx_eq!( _result.ev , in_1_odds * 50.0 );
}

// #[test]
fn ev_of_yahtzee_in_2_rolls() {
    let game = GameState{   rolls_remaining: 2, 
                            sorted_dievals: [1,2,3,4,5].into(),
                            sorted_open_slots: [YAHTZEE].into(), 
                            ..default()};
    let app = &mut AppState::new(&game);
    let _result = best_choice_ev(game, app);
    let in_2 = 0.01263; //https://www.datagenetics.com/blog/january42012/    
    // let in_1 = 6.0/7776.0; //0.00077; 
    assert_approx_eq!( rounded( _result.ev ,3), rounded( (in_2)*50.0, 3) );
}

// #[test]
fn ev_of_yahtzee_in_3_rolls() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{rolls_remaining: 3, 
                            sorted_open_slots: [YAHTZEE].into(), 
                            ..default() };
    let app = &mut AppState::new(&game);
    let _result = best_choice_ev(game, app);
    assert_approx_eq!( rounded( _result.ev, 2), rounded( 0.04603 * 50.0, 2) );
}

// #[test] 
fn ev_of_smstraight_in_1() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 1, 
                            sorted_open_slots: [SM_STRAIGHT].into(), 
                            ..default() };
    let app = &mut AppState::new(&game);
    let result = best_choice_ev(game, app);
    assert_eq!( rounded( result.ev / 30.0, 2), rounded( 0.1235 + 0.0309 , 2) );
}

// #[test]
fn ev_of_lgstraight_in_1() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{rolls_remaining: 1, sorted_open_slots: [LG_STRAIGHT].into(), ..default() };
    let app = &mut AppState::new(&game);
    let result = best_choice_ev(game, app);
    assert_eq!( rounded( result.ev  / 40.0, 4), rounded( 0.0309, 4) );
}

// #[test] 
fn ev_of_4ofakind_in_1() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{rolls_remaining: 1, sorted_open_slots: [FOUR_OF_A_KIND].into(), ..default() };
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
    let game = GameState{rolls_remaining: 1, sorted_open_slots: [THREE_OF_A_KIND].into(), ..default()};
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
fn print_misc() {
    // eprint!("{:?}", selection_ranges() );
    // eprint!("{:?}", all_selection_outcomes() );
    eprintln!("{:?}", selection_ranges() );
    // eprint!("{:?}", five_dice_combinations() );
    // eprint!("{:?}", die_index_combos() );
}

// #[test]
// fn unique_upper_totals_test() {
//     let s:Slots = [1,2,7].into();
//     assert_eq!(s.unique_upper_totals(), 16);
// }

// #[test]
fn unique_upper_totals_test() {
    let slots:SortedSlots = [1,2,4,5].into();
    let sorted_totals = slots.relevant_upper_totals().sorted().collect_vec();
    eprintln!("{:?} {}",sorted_totals, sorted_totals.len());
    // assert_eq!(sorted_totals, vec![48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63]);
}

// #[test]
// fn multi_cartesian_product_test() {
//     let it = repeat_n(1..=6,2).multi_cartesian_product().for_each(|x| eprintln!("{:?}",x));
// }

// #[test]
fn bench_test() {
    let game = GameState{   rolls_remaining: 0, 
                            sorted_open_slots: [SIXES, FOUR_OF_A_KIND, YAHTZEE].into(), 
                            upper_total: 63, 
                            ..default()};
    let app = &mut AppState::new(&game);
    let result = best_choice_ev(game, app);
    eprintln!("{:?}",result);
    eprintln!("{:?}",result);
    // assert_eq!(rounded(result.ev,2),  21.8);
} 

// #[test]
fn removed_test(){
    let s:SortedSlots = [1,3,5].into();
    let r = s.removed(1);
    assert_eq!(r,[3,5].into() );
}
 
// #[test]
fn counts_test(){
    let game = GameState::default();
    eprintln!("{:?}", game.counts() );
}

// #[test]
fn relevant_upper_totals_test(){
   let slots:SortedSlots = [1,2,4].into();
   let retval = slots.relevant_upper_totals() ;
   eprintln!("{:?}", retval.to().sorted() );
}

// // #[test]
// fn encode_sorted_test() {
//     let slots:Slots = [1,7,8,9,10,11,12,13].into(); 
//     let lhs=slots.encode_sorted_to_u16(); 
//     eprintln!("{:b}", lhs);
//     assert_eq!(lhs, 0b11111110000010);
// }

// // #[test]
// fn decode_u16_test() {
//     let lhs = Slots::decode_u16(0b11111110000010); 
//     eprintln!("{:?}", lhs);
//     assert_eq!(lhs, [1,7,8,9,10,11,12,13].into());
// }

// #[test]
fn print_out_cache(){
    let game = GameState { 
        rolls_remaining: 2,
        sorted_dievals: [3,4,4,6,6].into(), 
        sorted_open_slots:  [11].into(),
        upper_total: 0,
        yahtzee_bonus_avail: false 
    };
    let app = &mut AppState::new(&game);
    // build_cache(game,app);
    for entry in &app.ev_cache {
        print_state_choice(entry.0, *entry.1)    
    }
}

#[test]
fn new_bench_test() {
    let game = GameState{   rolls_remaining: 3,
                            sorted_open_slots: [1,2,8,9,10,11,12,13].into(), 
                            ..default() };
    let app = &mut AppState::new(&game);
    build_cache(game,app);
    let lhs = app.ev_cache.get(&game).unwrap();
    println!("lhs {:?}",lhs); 
    assert_eq!(lhs.ev,  138.66803);
} 


// #[test]
fn large_str8_test() {

    // this should be 7.41 per http://www-set.win.tue.nl/~wstomv/misc/yahtzee/osyp.php
    //    D	  7.37	11010	66443	 2	 0	F	11_

    let game = GameState { 
        rolls_remaining: 2,
        sorted_dievals: [3,4,4,6,6].into(), 
        sorted_open_slots:  [11].into(),
        upper_total: 0,
        yahtzee_bonus_avail: false 
    };
    let app = &mut AppState::new(&game);
    build_cache(game, app);
    for entry in &app.ev_cache {
        print_state_choice(entry.0, *entry.1)    
    }
    let lhs=app.ev_cache.get(&game).unwrap();
    println!("{:?}", lhs);
    println!("{:?}", lhs);
    assert_eq!(lhs.ev, 7.41);
}