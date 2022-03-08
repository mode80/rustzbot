use super::*;
// use test::Bencher;

// #[test]
fn score_slot_test() {
    assert_eq!(15, score_slot(FIVES,[1,2,5,5,5]));
}

#[test]
fn bench_test() {
    // let slots= array_vec!([u8;13] => 1,2,3,4,5,6,7,8,9,10,11,12,13);
    let game = GameState{   rolls_remaining: 0, 
                            sorted_open_slots: array_vec!([u8;13] => 6,8,12 ), 
                            sorted_dievals: [6,6,6,6,6], 
                            upper_bonus_deficit: 30, 
                            yahtzee_is_wild: false, };
    let _result = best_choice_ev(&game,&AppState::new(&game));
}

