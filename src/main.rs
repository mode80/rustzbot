#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

use std::{collections::{HashSet, HashMap}, vec, cmp::max};
use counter::Counter;

use cached::proc_macro::cached;
use itertools::{iproduct, Permutations, Itertools};
use ordered_float::OrderedFloat;


fn main() {
    //ad hoc testing code here for now 

    let dievals: [u8; 5] = [0, 0, 0, 0, 0];
    let die_combo: [bool; 5] = [true, true, true, true, true];

    let die_combos = die_combos();
    let die_index_combos = die_index_combos();

    // let it = score_upperbox(5, [1,4,2,5,5]) ;
    // let it = score_n_of_a_kind(3, [1,2,5,5,5]) ;
    // let it = straight_len([1,2,3,4,6]);
    // let it = score_sixes([1,6,3,4,6]);
    // let it = score_fullhouse([1,1,1,2,2]);
    // let it = score_chance([1,1,1,2,1]);
    // let it = score_yahtzee([1,1,1,2,1]);
    let it = SCORE_FNS[ACES]([1,1,2,3,1]);

    // let it = all_outcomes_for_rolling_n_dice(5);
    // println!("{}",fact(34));
    // println!("{}",n_take_r(13,13,true,false));
    // let roll_outcomes = all_outcomes_for_rolling_n_dice(5);

    // println!("{:#?}",die_index_combos.len());
    // let it:Vec<u128> = (1..=13).map(|r| n_take_r(13,r,false,false) ).collect::<>();

    println!("{:#?}", it);
}

const ACES:u8=1; 
const TWOS:u8=2; 
const THREES:u8=3; 
const FOURS:u8=4; 
const FIVES:u8=5; 
const SIXES:u8=6;

const THREE_OF_AKIND:u8=7; 
const FOUR_OF_AKIND:u8=8; 

const SM_STRAIGHT:u8=9; 
const LG_STRAIGHT:u8=10; 

const FULL_HOUSE:u8=11; 
const YAHTZEE:u8=12; 
const CHANCE:u8=13; 

const ALL_DICE:Vec<u8> = vec![0,1,2,3,4];
const UNROLLED_DIEVALS:[u8;5] = [0,0,0,0,0];
const SIDES:u8 = 6;

const SCORE_FNS:[fn(sorted_dievals:[u8;5])->u8;14] = [
    score_aces, // duplicate placeholder so indices align more intuitively with categories 
    score_aces, score_twos, score_threes, score_fours, score_fives, score_sixes, 
    score_3ofakind, score_4ofakind, score_sm_str8, score_lg_str8, score_fullhouse, score_yahtzee, score_chance, 
];

// rudimentary factorial suitable for our purposes here.. handles up to fact(34) */
fn fact(n: u128) -> u128{
    if n<=1 {1} else { n*fact(n-1) }
}

// count of arrangements that can be formed from r selections, chosen from n items, 
// where order DOES or DOESNT matter, and WITH or WITHOUT replacement, as specified
fn n_take_r(n:u128, r:u128, ordered:bool, with_replacement:bool)->u128{

    if !ordered { // we're counting "combinations" where order doesn't matter, so there are less of these 
        if with_replacement {
            fact(n+r-1) / (fact(r)*fact(n-1))
        } else { // no replacement
            fact(n) / (fact(r)*fact(n-r)) 
        }
    } else { // is ordered
        if with_replacement {
            n.pow(r as u32)
        } else { // no replacement
            fact(n) / fact(n-r)
        }
    }
}

// // the set of all ways to roll different dice, as represented by a collection of bool arrays
// // [[0,0,0,0,0], [1,0,0,0,0], [0,1,0,0,0], [0,0,1,0,0], [0,0,0,1,0], [0,0,0,0,1], [1,1,0,0,0],...
// fn die_combos()-> [[bool;5];64] {

//     let mut them = [[false;5];64] ;
//     let mut j = 0;
//     let false_true = [false, true];
//     for i in false_true {
//         for ii in false_true {
//             for iii in false_true {
//                 for iv in false_true {
//                     for v in false_true {
//                         them[j] = [i,ii,iii,iv,v]; 
//                         j+=1;
//                     }
//                 }
//             }
//         }
//     }
//     them
// }


// // the set of all ways to roll different dice, as represented by a collection of indice vecs 
#[cached]
fn die_index_combos() -> Vec<Vec<u8>>  { // TODO rewrite this to return an iterator?

    let mut them = vec![Vec::<u8>::new()]; 
    for i in 0u8..=4 {
        them.push(vec![i]);
        for ii in 0..=4 {
            them.push(vec![i,ii]);
            for iii in 0..=4 {
                them.push(vec![i,ii,iii]);
                for iv in 0..=4 {
                    them.push(vec![i,ii,iii,iv]);
                    for v in 0..=4 {
                        them.push(vec![i,ii,iii,iv,v]);
                    }
                }
            }
        }
    }
    them
}


#[cached]
fn all_outcomes_for_rolling_n_dice(n:u8) -> Vec<Vec<u8>> {

    assert!(n<=5);

    let mut them = vec![Vec::<u8>::new()]; 
    for i in 1..=6 {
        if n==1 {them.push(vec![i])} else {
        for ii in 1..=6 {
            if n==2 {them.push(vec![i,ii])} else {
            for iii in 1..=6 {
                if n==3 {them.push(vec![i,ii,iii])} else {
                for iv in 1..=6 {
                    if n==4 {them.push(vec![i,ii,iii,iv])} else {
                    for v in 1..=6 {
                        if n==5 {them.push(vec![i,ii,iii,iv,v])} 
                    }}
                }}
            }}
        }}
    }
    them
}

#[cached]
fn score_upperbox(boxnum:u8, sorted_dievals:[u8;5])->u8{
   sorted_dievals.iter().filter(|x| **x==boxnum).sum()
}

#[cached]
fn score_n_of_a_kind(n:u8,sorted_dievals:[u8;5])->u8{
    let mut inarow=1; let mut maxinarow=1; let mut lastval=255; let mut sum=0; 
    for x in sorted_dievals {
        if x==lastval {inarow +=1} else {inarow=1}
        maxinarow = max(inarow,maxinarow);
        lastval = x;
        sum+=x;
    }
    if maxinarow>=n {sum} else {0}
}


#[cached]
fn straight_len(sorted_dievals:[u8;5])->u8 {
    let mut inarow=1; 
    let mut maxinarow=1; 
    let mut lastval=254; // stub
    for x in sorted_dievals {
        if x==lastval+1 {inarow+=1} else {inarow=1};
        maxinarow = max(inarow,maxinarow);
        lastval = x;
    } 
    maxinarow 
}

fn score_aces(sorted_dievals:       [u8;5])->u8{ score_upperbox(1,sorted_dievals) }
fn score_twos(sorted_dievals:       [u8;5])->u8{ score_upperbox(2,sorted_dievals) }
fn score_threes(sorted_dievals:     [u8;5])->u8{ score_upperbox(3,sorted_dievals) }
fn score_fours(sorted_dievals:      [u8;5])->u8{ score_upperbox(4,sorted_dievals) }
fn score_fives(sorted_dievals:      [u8;5])->u8{ score_upperbox(5,sorted_dievals) }
fn score_sixes(sorted_dievals:      [u8;5])->u8{ score_upperbox(6,sorted_dievals) }

fn score_3ofakind(sorted_dievals:   [u8;5])->u8{ score_n_of_a_kind(3,sorted_dievals) }
fn score_4ofakind(sorted_dievals:   [u8;5])->u8{ score_n_of_a_kind(4,sorted_dievals) }
fn score_sm_str8(sorted_dievals:    [u8;5])->u8{ if straight_len(sorted_dievals) >= 4 {30} else {0} }
fn score_lg_str8(sorted_dievals:    [u8;5])->u8{ if straight_len(sorted_dievals) >= 5 {40} else {0} }

// The official rule is that a Full House is "three of one number and two of another"
fn score_fullhouse(sorted_dievals:[u8;5]) -> u8 { 
    let counts = sorted_dievals.iter().collect::<Counter<_>>().most_common_ordered(); //sorted(list(Counter(sorted_dievals).values() ))
    if counts.len()==2 && (counts[0].1==3 && counts[1].1==2) {25} else {0}
}

fn score_chance(sorted_dievals:[u8;5])->u8 { sorted_dievals.iter().sum()  }
fn score_yahtzee(sorted_dievals:[u8;5])->u8 { 
    let deduped=sorted_dievals.iter().dedup().collect_vec();
    if deduped.len()==1 {50} else {0} 
}

// reports the score for a set of dice in a given slot w/o regard for exogenous gamestate (bonuses, yahtzee wildcards etc)
fn score_slot(slot_index:usize, sorted_dievals:[u8;5])->u8{
    SCORE_FNS[slot_index](sorted_dievals) 
}


// returns the best slot and corresponding ev for final dice, given the slot possibilities and other relevant state 
fn best_slot_ev(sorted_open_slots:Vec<u8>, sorted_dievals:[u8;5], upper_bonus_deficit:u8, yahtzee_is_wild:bool) -> (u8,OrderedFloat<f32>) {

    let slot_sequences = sorted_open_slots.iter().permutations(sorted_open_slots.len()); 
    let mut evs:HashMap<OrderedFloat<f32>,Vec<&u8>> = HashMap::new();  // TODO consider faster hash function or BHashMap blah blah
    for slot_sequence in slot_sequences{
        let mut total:OrderedFloat<f32> = 0.0.into() ;
        let head_slot = *slot_sequence[0];
        let upper_deficit_now = upper_bonus_deficit ;

        let mut head_ev = SCORE_FNS[head_slot as usize](sorted_dievals); // score slot itself w/o regard to game state adjustments
        let yahtzee_rolled = sorted_dievals[0]==sorted_dievals[4]; // go on to adjust the raw ev for exogenous game state factors
        if yahtzee_rolled && yahtzee_is_wild { 
            head_ev+=100; // extra yahtzee bonus per rules
            if head_slot==SM_STRAIGHT {head_ev=30}; // extra yahtzees are valid in any lower slot per wildcard rules
            if head_slot==LG_STRAIGHT {head_ev=40}; 
            if head_slot==FULL_HOUSE {head_ev=25}; 
        }
        if head_slot <=SIXES && upper_deficit_now>0 && head_ev>0 { 
            if head_ev >= upper_deficit_now {head_ev+=35}; // add upper bonus when needed total is reached
            upper_deficit_now = max(upper_deficit_now - head_ev, 0) ;
        }
        total = total + OrderedFloat(head_ev.into());

        if slot_sequence.len() > 1 { // proceed to also score remaining slots
            let wild_now = if head_slot==YAHTZEE && yahtzee_rolled {true} else {yahtzee_is_wild};
            let tail_slots = slot_sequence[1..].iter().sorted();
            let tail_ev = 0.0; //TODO! ev_for_state(tail_slots, None, 3, upper_deficit_now, wild_now) # <---------
            total += tail_ev;
        }
        evs[&total] = slot_sequence;
    }

    let best_ev = evs.keys().max().unwrap();//max_by(|a, b| a.partial_cmp(b).unwrap()); // slot is a choice -- use max ev // rust floats can't be compared normally
    let best_sequence = evs[best_ev];
    let best_slot = best_sequence[0];

    return (*best_slot, *best_ev);
}

// returns the best selection of dice and corresponding ev, given slot possibilities and any existing dice and other relevant state 
fn best_dice_ev(sorted_open_slots:Vec<u8>, sorted_dievals:[u8;5], rolls_remaining:u8, upper_bonus_deficit:u8, yahtzee_is_wild:bool) -> (Vec<u8>,OrderedFloat<f32>){ 

    let selection_evs:HashMap<OrderedFloat<f32>,Vec<u8>> = HashMap::new(); 
    let die_combos:Vec<Vec<u8>> = vec![];
    if rolls_remaining==3{ //# we must select all dice on the first roll
        let sorted_dievals = UNROLLED_DIEVALS;
        let die_combos = vec![ALL_DICE] ;
    } else { //  # otherwise we must try all possible combos
        let die_combos= die_index_combos();
    }

    for selection in die_combos.iter(){ 
        let total:f32 = 0.0;
        let outcomes = all_outcomes_for_rolling_n_dice(selection.len() as u8);
        for outcome in outcomes{ 
            //###### HOT CODE PATH #######
            let mut newvals=sorted_dievals.clone();
            for (i, j) in selection.iter().enumerate() { 
                newvals[*j as usize]=outcome[i];    // TODO performance implications of the cast?
            }
            let sorted_newvals = newvals.iter().sorted().collect_vec();
            let ev = 0.0; // TODO! ev_for_state(sorted_open_slots, sorted_newvals, rolls_remaining-1, upper_bonus_deficit, yahtzee_is_wild)
            total += ev
            //############################
        }
        let avg_ev = total/outcomes.len() as f32; // outcomes are not a choice -- track average ev
        selection_evs[&OrderedFloat(avg_ev)] = *selection ;
    }
    
    let best_ev = selection_evs.keys().max().unwrap(); // selection is a choice -- track max ev
    let best_selection = selection_evs[best_ev] ;
    (best_selection, *best_ev)

}
