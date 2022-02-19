#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

use std::{collections::{HashSet, HashMap}, vec, cmp::max};
use counter::Counter;

use cached::proc_macro::cached;
use itertools::{iproduct, Permutations, Itertools, Combinations};
use ordered_float::OrderedFloat;


fn main() {
    //ad hoc testing code here for now 

    let dievals: [u8; 5] = [0, 0, 0, 0, 0];
    let die_combo: [bool; 5] = [true, true, true, true, true];

    // let die_combos = die_combos();

    // let it = score_upperbox(5, [1,4,2,5,5]) ;
    // let it = score_n_of_a_kind(3, [1,2,5,5,5]) ;
    // let it = straight_len([1,2,3,4,6]);
    // let it = score_sixes([1,6,3,4,6]);
    // let it = score_fullhouse([1,1,1,2,2]);
    // let it = score_chance([1,1,1,2,1]);
    // let it = score_yahtzee([1,1,1,2,1]);
    // let it = SCORE_FNS[ACES as usize]([1,1,2,3,1]);
    // let it = best_dice_ev(&[CHANCE], [6,6,6,6,6], 1, INIT_DEFICIT, false);
    let it = ev_for_state(&[1,2,3,4,5,6,7,8,9,10,11,12,13], [1,1,6,6,6], 3, INIT_DEFICIT, false);
    // let it = die_index_combos();

    // let it = all_outcomes_for_rolling_n_dice(5);
    // println!("{}",fact(34));
    // println!("{}",n_take_r(13,13,true,false));
    // let roll_outcomes = all_outcomes_for_rolling_n_dice(5);

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

const UNROLLED_DIEVALS:[u8;5] = [0,0,0,0,0];
const SIDES:u8 = 6;
const INIT_DEFICIT:u8 = 63;

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


// the set of all ways to roll different dice, as represented by a collection of indice vecs 
#[cached]
fn die_index_combos() -> Vec<Vec<u8>>  { 
    let mut them:Vec<Vec<u8>> = (0..=4).combinations(0).collect_vec();
    them.append(& mut (0..=4).combinations(1).collect_vec());
    them.append(& mut (0..=4).combinations(2).collect_vec());
    them.append(& mut (0..=4).combinations(3).collect_vec());
    them.append(& mut (0..=4).combinations(4).collect_vec());
    them.append(& mut (0..=4).combinations(5).collect_vec());
    them 
}


#[cached]
fn all_outcomes_for_rolling_n_dice(n:u8) -> Vec<Vec<u8>> {

    assert!(n<=5);

    let mut them = vec![]; 
    if n==0 {them.push(vec![]) } else {
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
    }}
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
fn best_slot_ev(sorted_open_slots:&[u8], sorted_dievals:[u8;5], upper_bonus_deficit:u8, yahtzee_is_wild:bool) -> (u8,f32) {

    let slot_sequences = sorted_open_slots.iter().permutations(sorted_open_slots.len());
    let mut evs:HashMap<OrderedFloat<f32>,Vec<u8>> = HashMap::new();  // TODO consider faster hash function or BHashMap blah blah
    for slot_sequence in slot_sequences{
        let mut total:f32 = 0.0;
        let slot_sequence:Vec<u8> = slot_sequence.iter().copied().copied().collect(); // wtf rust?
        let head_slot = slot_sequence[0];
        let mut upper_deficit_now = upper_bonus_deficit ;

        let mut head_ev = SCORE_FNS[head_slot as usize](sorted_dievals); // score slot itself w/o regard to game state adjustments
        let yahtzee_rolled = sorted_dievals[0]==sorted_dievals[4]; // go on to adjust the raw ev for exogenous game state factors
        if yahtzee_rolled && yahtzee_is_wild { 
            if head_slot==SM_STRAIGHT {head_ev=30}; // extra yahtzees are valid in any lower slot per wildcard rules
            if head_slot==LG_STRAIGHT {head_ev=40}; 
            if head_slot==FULL_HOUSE {head_ev=25}; 
            head_ev+=100; // extra yahtzee bonus per rules
        }
        if head_slot <=SIXES && upper_deficit_now>0 && head_ev>0 { 
            if head_ev >= upper_deficit_now {head_ev+=35}; // add upper bonus when needed total is reached
            upper_deficit_now = max(upper_deficit_now - head_ev, 0) ;
        }
        total += head_ev as f32;

        if slot_sequence.len() > 1 { // proceed to also score remaining slots
            let wild_now = if head_slot==YAHTZEE && yahtzee_rolled {true} else {yahtzee_is_wild};
            let tail_slots = slot_sequence[1..].to_vec();
            let tail_ev = ev_for_state(&tail_slots, UNROLLED_DIEVALS, 3, upper_deficit_now, wild_now); // <---------
            total += tail_ev as f32;
        }
        let total_index:OrderedFloat<f32> = OrderedFloat(total);
        evs.insert(total_index, slot_sequence);
    }

    let best_ev = *evs.keys().max().unwrap();
    let best_sequence = evs.get(&best_ev).unwrap();
    let best_slot = best_sequence[0];
    let ev_f32 = best_ev.into_inner();

    (best_slot,ev_f32)
}

// returns the best selection of dice and corresponding ev, given slot possibilities and any existing dice and other relevant state 
fn best_dice_ev(sorted_open_slots:&[u8], sorted_dievals:[u8;5], rolls_remaining:u8, upper_bonus_deficit:u8, yahtzee_is_wild:bool) -> (Vec<u8>,f32){ 

    let mut selection_evs:HashMap<OrderedFloat<f32>,Vec<u8>> = HashMap::new(); 
    let mut die_combos:Vec<Vec<u8>> = vec![];
    if rolls_remaining==3{ //# we must select all dice on the first roll
        let sorted_dievals = UNROLLED_DIEVALS;
        die_combos.push(vec![0,1,2,3,4]) ; //all dice
    } else { //  # otherwise we must try all possible combos
        die_combos= die_index_combos();
    }

    for selection in die_combos.iter(){ 
        let mut total:f32 = 0.0;
        let outcomes = all_outcomes_for_rolling_n_dice(selection.len() as u8);
        let outcomeslen=outcomes.len();
        for outcome in outcomes{ 
            //###### HOT CODE PATH #######
            let mut newvals=sorted_dievals;
            for (i, j) in selection.iter().enumerate() { 
                newvals[*j as usize]=outcome[i];    // TODO performance implications of the cast?
            }
            let sorted_newvals = newvals.iter().sorted().cloned().collect_vec().try_into().unwrap(); // oy
            let ev =  ev_for_state(sorted_open_slots, sorted_newvals, rolls_remaining-1, upper_bonus_deficit, yahtzee_is_wild);
            total += ev;
            //############################
        }
        let avg_ev = total/outcomeslen as f32; // outcomes are not a choice -- track average ev
        let evs_index = OrderedFloat(avg_ev);
        selection_evs.insert(evs_index , selection.clone()) ;
    }
    
    let best_ev = *selection_evs.keys().max().unwrap(); // selection is a choice -- track max ev
    let best_selection = selection_evs.get(&best_ev).unwrap().clone() ;
    (best_selection, best_ev.into_inner())

}

// returns a hashable key for relevant state parameters 
fn key_for_state(sorted_open_slots:&[u8], sorted_dievals:[u8;5], rolls_remaining:u8, upper_bonus_deficit:u8, yahtzee_is_wild:bool) -> String { 
    // TODO optimize this for size with struct? bitmasked u128?
    let mut key = String::with_capacity(35); 
    let mut deficit_now = upper_bonus_deficit; 
    for slot in sorted_open_slots{ key.push_str(&slot.to_string()); }
    for die in sorted_dievals{ key.push_str(&die.to_string()); }
    key.push_str(&rolls_remaining.to_string());
    if upper_bonus_deficit > 0 && sorted_open_slots[0]>SIXES{ //trim the cachable state by ignoring upper total variations when no more upper slots are left
        deficit_now=63
    }
    key.push_str(&deficit_now.to_string());
    key.push_str(&(yahtzee_is_wild as u8).to_string());
    key
}

// returns the additional expected value to come, given relevant game state.'''
#[cached(key = "String", convert = r#"{ key_for_state(&sorted_open_slots,sorted_dievals,rolls_remaining,upper_bonus_deficit,yahtzee_is_wild) }"#)]
fn ev_for_state(sorted_open_slots:&[u8], sorted_dievals:[u8;5], rolls_remaining:u8, upper_bonus_deficit:u8, yahtzee_is_wild:bool) -> f32{ 
    // global progress_bar, log, done, ev_cache

    // if progress_bar is None: 
    //     lenslots=len(sorted_open_slots)
    //     open_slot_combos = sum(n_take_r(lenslots,r,False,False) for r in fullrange(1,lenslots)) 
    //     done = set(s for s,_,r,_,_ in ev_cache.keys() if r==3)
    //     progress_bar = tqdm(initial=len(done), total=open_slot_combos, smoothing=0.0) 

    let ev:f32;
    if rolls_remaining == 0 {
        let result = best_slot_ev(sorted_open_slots, sorted_dievals, upper_bonus_deficit, yahtzee_is_wild);                 // <-----------------
        ev = result.1;
    } else { 
        let result = best_dice_ev(sorted_open_slots, sorted_dievals, rolls_remaining, upper_bonus_deficit, yahtzee_is_wild);  // <-----------------
        ev = result.1;
    }
            
    // log_line = f'{rolls_remaining:<2}\t{str(_):<15}\t{ev:6.2f}\t{str(sorted_dievals):<15}\t{upper_bonus_deficit:<2}\t{yahtzee_is_wild}\t{str(sorted_open_slots)}' 
    println!( "rolls_remaining: {}\t result: _ \t ev: {:.2}  \t dievals:{:?}\t deficit: {}\t wild: {}\t slots: {:?}", 
             rolls_remaining,                    ev, sorted_dievals, upper_bonus_deficit, yahtzee_is_wild, sorted_open_slots);
    // progress_bar.write(log_line)
    // print(log_line,file=log)

    // if rolls_remaining==3: # periodically update progress and save
    //     if sorted_open_slots not in done:
    //         done.add(sorted_open_slots)
    //         progress_bar.update(1) 
    //         if len(done) % 80 == 0 :
    //             with open('ev_cache.pkl','wb') as f: pickle.dump(ev_cache,f)
 
    ev 
}
