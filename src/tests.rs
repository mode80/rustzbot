#![allow(dead_code)]

use std::{io::BufWriter, thread::JoinHandle, sync::mpsc, time::{self, Instant}};

use assert_approx_eq::assert_approx_eq;
use itertools::Chunk;

use super::*;

fn rounded(it:f32,places:i32) -> f32{
    let e = 10_f32.powi(places);
    (it * e).round() / e 
}

// #[test]
fn test_sort() {
    let mut sortable:Slots= [2,6,5,1,5].into();
    let presorted:Slots = [1,2,5,5,6].into();
    sortable.sort();
    assert_eq!(sortable.data, presorted.data);
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
                            sorted_open_slots: [YAHTZEE].into(),// array_vec!([Dieval;13] => YAHTZEE ), 
                            sorted_dievals: Default::default(), 
                            upper_bonus_deficit: INIT_DEFICIT , yahtzee_is_wild: false, };
    let app = &mut AppState::new(&game);
    let _result = best_choice_ev(game, app);
    let in_1_odds = 6.0/7776.0; 
    assert_approx_eq!( _result.ev , in_1_odds * 50.0 );
}

// #[test]
fn ev_of_yahtzee_in_2_rolls() {
    let game = GameState{   rolls_remaining: 2, 
                            sorted_open_slots: [YAHTZEE].into(), 
                            sorted_dievals: Default::default(), 
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
                            sorted_open_slots: [YAHTZEE].into(), 
                            sorted_dievals: Default::default(), 
                            upper_bonus_deficit: INIT_DEFICIT , yahtzee_is_wild: false, };
    let app = &mut AppState::new(&game);
    let _result = best_choice_ev(game, app);
    assert_approx_eq!( rounded( _result.ev, 2), rounded( 0.04603 * 50.0, 2) );
}

// #[test] 
fn ev_of_smstraight_in_1() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 1, 
                            sorted_open_slots: [SM_STRAIGHT].into(), 
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
                            sorted_open_slots: [LG_STRAIGHT].into(), 
                            sorted_dievals: Default::default(), 
                            upper_bonus_deficit: INIT_DEFICIT , yahtzee_is_wild: false, };
    let app = &mut AppState::new(&game);
    let result = best_choice_ev(game, app);
    assert_eq!( rounded( result.ev  / 40.0, 4), rounded( 0.0309, 4) );
}

// #[test] 
fn ev_of_4ofakind_in_1() {
// see https://www.yahtzeemanifesto.com/yahtzee-odds.php 
    let game = GameState{   rolls_remaining: 1, 
                            sorted_open_slots: [FOUR_OF_A_KIND].into(), 
                            sorted_dievals: Default::default(), 
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
                            sorted_open_slots: [THREE_OF_A_KIND].into(), 
                            sorted_dievals: Default::default(), 
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
fn progress_eta_test() {
    let game = GameState{   rolls_remaining: 3, 
                            sorted_open_slots: [1,2,3,4,5,6,7,8,9,10,11,12,13].into(), 
                            sorted_dievals: Default::default(), 
                            upper_bonus_deficit: 30, 
                            yahtzee_is_wild: false, };
    let app = &mut AppState::new(&game);
    let result = best_choice_ev(game, app);
}

// #[test]
fn test_permutations_within() {

    let a:Slots = [1,2,3,4,5].into();
    for perm in a.permutations_within(1,3) { 
        println!("{}", perm); 
    }; 
}

// #[test]
// fn test_truncate() {
//     let mut l:Slots = [1,2,3,4,5].into();
//     l.truncate(3);
//     let r:Slots = [1,2,3].into();
//     assert_eq!(l,r);
//  }

// #[test]
fn test_subset() {
    let slots:Slots = [1,2,3,4].into();
    let l:Slots = [2,3].into();
    assert_eq!(l,slots.subset(1,2));
    let l:Slots = [2,3,4].into();
    assert_eq!(l,slots.subset(1,4));
  }


// #[test]
fn test_threaded_permutations() {
    let hm = Arc::new(Mutex::new(FxHashMap::<Slots,u8>::default()));
    let mut ret = 0;
    let slots:Slots = [1,2,3,4,5,6,7,8].into();
    let mut span_lens = (2..=slots.len/2).collect_vec();
    span_lens.push(slots.len);
    // lens = vec![4,8];
    for span_len in span_lens { 
        for offset in 0..span_len { 
            let hm = hm.clone();
            let size_hm = hm.lock().unwrap().clone();
            let span_count = slots.len / span_len - if offset>0 {1} else {0}; 
            ret+= (0..span_count).into_par_iter().map(move |i|
            { // THREADS | each thread parallel processes a non-overlapping span
                let thread_hm = &mut size_hm.clone();
                let mut tot =0;
                let span_start = i * span_len as u8;
                for perm in slots.permutations_within(span_start, span_len) { 
                    if let Some(s) = thread_hm.get(&perm) {
                        eprintln!("{} {} {} {} C", perm, s, i*span_len, span_len) ;
                        sleep(Duration::new(0,1000));
                        tot += *s as u64;
                    } else {
                        let s = perm.into_iter().sum() ;
                        thread_hm.insert(perm, s);
                        eprintln!("{} {} {} {}", perm, s, i*span_len, span_len) ;
                        sleep(Duration::new(0,1000));
                        tot += s as u64;
                    }
                }; 
                hm.lock().unwrap().extend(thread_hm.iter());
                tot
            }).sum::<u64>();
        }
    }
    eprintln!("{}", ret); // 1451520 2.21s on debug 
 }

// #[test]
fn test_threaded_subsets() {

    // NOTE bottom up (not recursive) approach means we can skip the cache lookup and sorting the key, since every new calc will be fresh?

    const CORES:usize = 8;

    let full_set:Slots = [1,2,3,4,5].into();
    let mut cached_val:u8;
    for set_len in 1..=full_set.len{ // each length 
        let (tx, rx) = mpsc::channel();
        let now = Instant::now();
        for i in 0..=(full_set.len-set_len) { // each slot_set (of above length)
            let slot_set = full_set.subset(i,set_len);
            let chunk_size = fact(slot_set.len) as usize / CORES + 1 ; // +1 to "round up" 
            println!("{}", chunk_size);
            for chunk in slot_set.permutations().chunks(chunk_size).into_iter(){ // each chunk -- one per core
                let slot_set_perms = chunk.collect_vec(); // TODO some way to pass iterator into thread instead?
                let tx = tx.clone();
                thread::spawn(move ||{
                    let mut chunk_best_result:ChoiceEV = Default::default();
                    let mut chunk_best_perm:Slots = Default::default();
                    for slot_perm in slot_set_perms { // each permutation
                        let score:u8 = slot_perm.into_iter().sum(); sleep(Duration::new(1,0)); // not a real score calc, just simulating
                        if score as f32 > chunk_best_result.ev { 
                            chunk_best_result.choice = slot_perm.get(0); 
                            chunk_best_result.ev = score as f32; 
                            chunk_best_perm = slot_perm;
                        } // remember best 
                    }; // end for each permutation in chunk
                    tx.send((chunk_best_perm, chunk_best_result)).unwrap();
                }); //end thread 
            } // end for each chunk
        } // end for each slot_set 
        drop(tx); // the cloned transmitter must be explicitly dropped since it never sends 
        let mut span_best_result:ChoiceEV = Default::default();
        let mut span_best_perm:Slots = Default::default();
        for rcvd in &rx { 
            if rcvd.1.ev > span_best_result.ev {span_best_result = rcvd.1; span_best_perm = rcvd.0};
        }
        println!("{} {:?} {:.2?}",span_best_perm, span_best_result, now.elapsed()); 
    } // end for each length

 } // end fn


// #[test]
fn unique_upper_deficits_test() {
    let slots:Slots = [1,2,4,5].into();
    let mut sorted_totals = slots.upper_total_deficits();
    sorted_totals.sort_unstable();
    eprintln!("{:?} {}",sorted_totals, sorted_totals.len());
    // assert_eq!(sorted_totals, vec![48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63]);
}

// #[test]
// fn multi_cartesian_product_test() {
//     let it = repeat_n(1..=6,2).multi_cartesian_product().for_each(|x| eprintln!("{:?}",x));
// }

// #[test]
fn all_selection_outcomes_test() { //TODO std::SIMD ?
    for outcome in all_selection_outcomes(){
        let mut sortedvals = outcome.dievals; 
        sortedvals.sort();
        eprintln!("{} {} {} {}",outcome.dievals, outcome.mask, outcome.arrangements, sortedvals);
    }
}

// #[test]
fn bench_test() {
    let game = GameState{   rolls_remaining: 3, 
                            sorted_open_slots: [SIXES, FOUR_OF_A_KIND, YAHTZEE].into(), 
                            sorted_dievals: [1,2,3,4,5].into(), 
                            upper_bonus_deficit: 30, 
                            yahtzee_is_wild: false, };
    let app = &mut AppState::new(&game);
    let result = best_choice_ev(game, app);
    eprintln!("{:?}",result);
    eprintln!("{:?}",result);
    // assert_eq!(rounded(result.ev,2),  21.8);
} 

// #[test]
fn removed_test(){
    let s:Slots = [1,3,5].into();
    let r = s.removed(1);
    assert_eq!(r,[3,5].into() );
}
 
// #[test]
fn swap_test(){
    let mut s:Slots = [0,1,2,3,4,5,6,7,8,9,10,11,12].into(); 
    s.swap(5,10);
    assert_eq!(s,[0,1,2,3,4,10,6,7,8,9,5,11,12].into());
}
// TODO see https://internals.rust-lang.org/t/bit-twiddling-pre-rfc/7072

// #[test]
fn test_permutations() {

    let a:Slots = [1,2,3,4,5,6,7,8,9,10].into();
    for perm in a.permutations() { 
        println!("{}", perm); 
    }; 
}

// #[test]
fn new_bench_test() {
    let game = GameState{   rolls_remaining: 3,
                            sorted_open_slots: [1,7,8,9,10,11,12,13].into(), 
                            sorted_dievals: [0,0,0,0,0].into(), 
                            upper_bonus_deficit: 0, 
                            yahtzee_is_wild: false, };
    let app = &mut AppState::new(&game);
    build_cache(game,app);
    let lhs = app.ev_cache.get(&game).unwrap();
    println!("lhs {:?}",lhs); 
    // assert_eq!(lhs.ev,  21.80351);
} 

// #[test]
fn build_cache_test() {
    let game = GameState{   rolls_remaining: 3,
                            sorted_open_slots: [1,3,5].into(), 
                            sorted_dievals: [0,0,0,0,0].into(), 
                            upper_bonus_deficit: 63, 
                            yahtzee_is_wild: false, };
    let app1 = &mut AppState::new(&game);
    let rhs = best_choice_ev(game, app1);
    let app2 = &mut AppState::new(&game);
    build_cache(game,app2);
    let lhs = app2.ev_cache.get(&game).unwrap();
    eprintln!("lhs {:?}",lhs);
    eprintln!("rhs {:?}",rhs); eprintln!("rhs {:?}",rhs);
    assert_eq!(lhs.ev,  rhs.ev);
}

#[test]
fn main_test(){

    let game = GameState::default();
    let app = & mut AppState::new(&game);
    build_cache(game,app);
    app.save_cache();

}