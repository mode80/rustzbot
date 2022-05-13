#![allow(dead_code)]

use std::io::BufWriter;

use assert_approx_eq::assert_approx_eq;

use super::*;

fn print_state_choice(state: &GameState, choice_ev:ChoiceEV){
    if state.rolls_remaining==0 {
        println!("S {:_>6.2?} {:_^5} {:2?} {} {:2?} {} {:_<29}",
            choice_ev.ev, choice_ev.choice, state.rolls_remaining, state.sorted_dievals, state.upper_total, 
            if state.yahtzee_bonus_avail {"Y"}else{"_"}, state.sorted_open_slots.to_string()); 
    } else {
        println!("D {:_>6.2?} {:05b} {:2?} {} {:2?} {} {:_<29}",
            choice_ev.ev, choice_ev.choice, state.rolls_remaining, state.sorted_dievals, state.upper_total, 
            if state.yahtzee_bonus_avail {"Y"}else{"_"}, state.sorted_open_slots.to_string()); 
    };
}

fn find_best_choice(app:&mut App) -> ChoiceEV{
    app.build_cache();
    app.best_choice_ev()
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
    assert_eq!(0, Score::slot_with_dice(SlotID::FULL_HOUSE,[1,2,5,5,5].into() ));
    assert_eq!(25, Score::slot_with_dice(SlotID::FULL_HOUSE,[2,2,5,5,5].into()));
    assert_eq!(0, Score::slot_with_dice(SlotID::FULL_HOUSE,[0,0,5,5,5].into()));
    assert_eq!(0, Score::slot_with_dice(SlotID::YAHTZEE,[4,5,5,5,5].into()));
    assert_eq!(50, Score::slot_with_dice(SlotID::YAHTZEE,[1,1,1,1,1].into()));
}


// #[test]
fn ev_of_yahtzee_in_1_roll() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 1, 
                            sorted_open_slots: [SlotID::YAHTZEE].into(), 
                            sorted_dievals: [1,2,3,4,5].into(),
                            ..default() };
    let app = &mut App::new(game);
    let _result = find_best_choice(app);
    let in_1_odds = 6.0/7776.0; 
    assert_approx_eq!( _result.ev , in_1_odds * 50.0 );
}

// #[test]
fn ev_of_yahtzee_in_2_rolls() {
    let game = GameState{   rolls_remaining: 2, 
                            sorted_dievals: [1,2,3,4,5].into(),
                            sorted_open_slots: [SlotID::YAHTZEE].into(), 
                            ..default()};
    let app = &mut App::new(game);
    let _result = find_best_choice(app);
    let in_2 = 0.01263; //https://www.datagenetics.com/blog/january42012/    
    // let in_1 = 6.0/7776.0; //0.00077; 
    assert_approx_eq!( rounded( _result.ev ,3), rounded( (in_2)*50.0, 3) );
}

// #[test]
fn ev_of_yahtzee_in_3_rolls() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{rolls_remaining: 3, 
                            sorted_open_slots: [SlotID::YAHTZEE].into(), 
                            ..default() };
    let app = &mut App::new(game);
    let _result = find_best_choice(app);
    assert_approx_eq!( rounded( _result.ev, 2), rounded( 0.04603 * 50.0, 2) );
}

// #[test] 
fn straight_len_test() {
    assert_eq!(Score::straight_len([1,1,1,1,1].into()),1);
}

// #[test] 
fn ev_of_smstraight_in_1() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 0, 
                            sorted_open_slots: [SlotID::SM_STRAIGHT].into(), 
                            sorted_dievals:[1,1,1,1,1].into(),
                            ..default() };
    let app = &mut App::new(game);
    let result = find_best_choice(app);
    for entry in &app.ev_cache {
        print_state_choice(entry.0, *entry.1)    
    }
    assert_eq!( rounded( result.ev / 30.0, 2), rounded( 0.1235 + 0.0309 , 2) );
}

// #[test]
fn ev_of_lgstraight_in_1() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{rolls_remaining: 1, sorted_open_slots: [SlotID::LG_STRAIGHT].into(), ..default() };
    let app = &mut App::new(game);
    let result = find_best_choice(app);
    assert_eq!( rounded( result.ev  / 40.0, 4), rounded( 0.0309, 4) );
}

// #[test] 
fn ev_of_4ofakind_in_1() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{rolls_remaining: 1, sorted_open_slots: [SlotID::FOUR_OF_A_KIND].into(), ..default() };
    let app = &mut App::new(game);
    let result = find_best_choice(app);
    assert_eq!( 
        rounded( result.ev  / 17.5, 3), // we divide EV by average dice-total to get odds
        rounded( 0.0193+0.00077, 3) //our 3 of a kind includes 4 of a kind & yahtzee
    );
}

// #[test] 
fn ev_of_3ofakind_in_1() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{rolls_remaining: 1, sorted_open_slots: [SlotID::THREE_OF_A_KIND].into(), ..default()};
    let app = &mut App::new(game);
    let result = find_best_choice(app); 
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
// fn multi_cartesian_product_test() {
//     let it = repeat_n(1..=6,2).multi_cartesian_product().for_each(|x| eprintln!("{:?}",x));
// }

// #[test]
fn bench_test() {
    let game = GameState{   rolls_remaining: 0, 
                            sorted_open_slots: [SlotID::SIXES, SlotID::FOUR_OF_A_KIND, SlotID::YAHTZEE].into(), 
                            upper_total: 63, 
                            ..default()};
    let app = &mut App::new(game);
    let result = find_best_choice(app);
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

// #[test]
fn print_out_cache(){
    let game : GameState = default() ;
    let app = &mut App::new(game);
    app.build_cache();
    for entry in &app.ev_cache {
        print_state_choice(entry.0, *entry.1)    
    }
}

// #[test]
fn known_values_test() {
    // this should be 20.73 per http://www-set.win.tue.nl/~wstomv/misc/yahtzee/osyp.php
    let game = GameState { 
        rolls_remaining: 2,
        sorted_dievals: [3,4,4,6,6].into(), 
        sorted_open_slots: [6,12].into(), 
       ..default()
    };
    let app = &mut App::new(game);
    app.build_cache();
    // for entry in &app.ev_cache {
    //     print_state_choice(entry.0, *entry.1)    
    // }
    let lhs=app.ev_cache.get(&game).unwrap();
    // println!("{:?}", lhs);
    // println!("{:?}", lhs);
    assert_eq!(rounded(lhs.ev,2), 20.73);
}

// #[test]
fn new_bench_test() {
    let game = GameState{   rolls_remaining: 2,
                            sorted_dievals: [3,4,4,6,6].into(), 
                            sorted_open_slots: [1,2,8,9,10,11,12,13].into(), 
                            ..default() };
    let app = &mut App::new(game);
    app.build_cache();
    let lhs = app.ev_cache.get(&game).unwrap();
    // println!("lhs {:?}",lhs); 
    assert_eq!(lhs.ev,  137.37492);
} 

#[test]
fn test_rust_bug() {

    let leaf_cache = [ChoiceEV::default(); 4_194_304];
    println!("success?");

}

