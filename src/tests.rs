use super::*;
// use test::Bencher;

// #[test]
fn score_slot_test() {
    assert_eq!(15, score_slot(FIVES,[1,2,5,5,5]));
}

#[test]
fn ev_for_state_test() {
    // let slots= array_vec!([u8;13] => 1,2,3,4,5,6,7,8,9,10,11,12,13);
    let game_state = &GameState{
        sorted_open_slots: array_vec!([u8;13] => 8,13),
        sorted_dievals: [1,2,5,5,5],
        rolls_remaining: 1,
        upper_bonus_deficit: INIT_DEFICIT,
        yahtzee_is_wild: false,
    };
    let slot_count=game_state.sorted_open_slots.len();
    let combo_count = (1..=slot_count).map(|r| n_take_r(slot_count as u128, r as u128,false,false) as u64 ).sum() ;
    let app_state = & mut AppState{
        progress_bar : Arc::new(RwLock::new(ProgressBar::new(combo_count))), 
        done : Arc::new(DashSet::new()) ,  
        ev_cache : Arc::new(DashMap::new()),
    };
    ev_for_state(game_state,app_state);
}


// #[test]
fn bench_test() {
    // let slots= array_vec!([u8;13] => 1,2,3,4,5,6,7,8,9,10,11,12,13);
    let slots= array_vec!([u8;13] => 7,13);
    let game_state = &GameState{
        sorted_open_slots: slots,
        sorted_dievals: [1,1,5,5,5],
        rolls_remaining: 1,
        upper_bonus_deficit: INIT_DEFICIT,
        yahtzee_is_wild: false,
    };
    let slot_count=game_state.sorted_open_slots.len();
    let combo_count = (1..=slot_count).map(|r| n_take_r(slot_count as u128, r as u128,false,false) as u64 ).sum() ;
    let app_state = & mut AppState{
        progress_bar : Arc::new(RwLock::new(ProgressBar::new(combo_count))), 
        done : Arc::new(DashSet::new()) ,  
        ev_cache : Arc::new(DashMap::new()),
    };
    ev_for_state(game_state,app_state);
}


// // #[bench]
// fn score_slot_bench(b: &mut Bencher) {
//     b.iter(best_dice_ev_test);
// }


