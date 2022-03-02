#![allow(dead_code)]
//#![allow(unused_variables)]
#![allow(unused_imports)]

use std::{collections::{HashSet, HashMap, BinaryHeap}, vec, cmp::max, sync::{Arc, Mutex, RwLock}};
use counter::Counter;

use cached::proc_macro::cached;
use dashmap::{DashMap, DashSet};
use itertools::Itertools;
use ordered_float::OrderedFloat;
use indicatif::ProgressBar;
use std::fmt::{Formatter, Display, Result};
use tinyvec::*;

fn main() {
    use SlotType::*;
    //ad hoc testing code here for now 

    /* setup game state */
    let game_state = &GameState{
            sorted_open_slots: ArrayVec::from([Aces,Twos,Threes,Fours,Fives,Sixes,ThreeOfAKind,FourOfAKind,SmStraight,LgStraight,FullHouse,Yahtzee,Chance]),
            sorted_dievals: UNROLLED_DIEVALS,
            rolls_remaining: 3,
            upper_bonus_deficit: INIT_DEFICIT,
            yahtzee_is_wild: false,
        };
    
    /* setup app state */
    let slot_count=game_state.sorted_open_slots.len();
    // let combo_count = (1..=slot_count).reduce(|accum,r| accum+n_take_r(slot_count as u128, r as u128 ,false,false) as usize).unwrap() ;
    let combo_count = (1..=slot_count).map(|r| n_take_r(slot_count as u128, r as u128 ,false,false) as u64 ).sum() ;
    let app_state = & mut AppState{
        progress_bar : Arc::new(RwLock::new(ProgressBar::new(combo_count))), 
        done : Arc::new(DashSet::new()) ,  // TODO try DashMap crate
        ev_cache : Arc::new(DashMap::new()),
    };

    /* do it */
    let it = best_dice_ev(game_state, app_state);
    
    println!("{:?}", it);
}

#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Clone, Copy)]
struct GameState{
    sorted_dievals:[u8;5], 
    rolls_remaining:u8, 
    upper_bonus_deficit:u8, 
    yahtzee_is_wild:bool,
    // sorted_open_slots:&'a [SlotType], 
    sorted_open_slots:ArrayVec<[SlotType;13]>, 
}

struct AppState{
    progress_bar:Arc<RwLock<ProgressBar>>, 
    done:Arc<DashSet<[SlotType;13]>>, 
    ev_cache:Arc<DashMap<GameState,f32>>,
    // log, 
}

#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Clone, Copy)]
enum SlotType {
    Stub=0,
    Aces=1, 
    Twos=2, 
    Threes=3, 
    Fours=4, 
    Fives=5, 
    Sixes=6,
    ThreeOfAKind=7, 
    FourOfAKind=8, 
    SmStraight=9, 
    LgStraight=10, 
    FullHouse=11, 
    Yahtzee=12, 
    Chance=13, 
}
// impl ToString for SlotType{
//     fn to_string(&self)->String{
//         (*self as u8).to_string()
//     }
// }
impl Default for SlotType {
    fn default() -> SlotType {
        SlotType::Stub
    }
}
impl Display for SlotType {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        // write!(f, "({}, {})", self.longitude, self.latitude)
        write!(f, "{}", *self as u8)
    }
}



 
const UNROLLED_DIEVALS:[u8;5] = [0,0,0,0,0];
const SIDES:u8 = 6;
const INIT_DEFICIT:u8 = 63;

const SCORE_FNS:[fn(sorted_dievals:[u8;5])->u8;14] = [
    score_aces, // duplicate placeholder so indices align more intuitively with categories 
    score_aces, score_twos, score_threes, score_fours, score_fives, score_sixes, 
    score_3ofakind, score_4ofakind, score_sm_str8, score_lg_str8, score_fullhouse, score_yahtzee, score_chance, 
];

/// rudimentary factorial suitable for our purposes here.. handles up to fact(34) */
fn fact(n: u128) -> u128{
    if n<=1 {1} else { n*fact(n-1) }
}

/// count of arrangements that can be formed from r selections, chosen from n items, 
/// where order DOES or DOESNT matter, and WITH or WITHOUT replacement, as specified
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


/// the set of all ways to roll different dice, as represented by a collection of indice vecs 
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

fn score_upperbox(boxnum:u8, sorted_dievals:[u8;5])->u8{
   sorted_dievals.iter().filter(|x| **x==boxnum).sum()
}

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

/// reports the score for a set of dice in a given slot w/o regard for exogenous gamestate (bonuses, yahtzee wildcards etc)
#[cached]
fn score_slot(slot:SlotType, sorted_dievals:[u8;5])->u8{
    SCORE_FNS[slot as usize](sorted_dievals) 
}


/// returns the best slot and corresponding ev for final dice, given the slot possibilities and other relevant state 
fn best_slot_ev(game:&GameState, app: &mut AppState) -> (SlotType,f32) {

    use SlotType::*;
    let slot_sequences = game.sorted_open_slots.into_iter().permutations(game.sorted_open_slots.len()); // TODO a version of this that doesn't allocate with vecs
    let mut evs:HashMap<OrderedFloat<f32>,ArrayVec<[SlotType;13]>> = HashMap::new();  
    let mut best_ev:OrderedFloat<f32> = OrderedFloat(0.0); 
    for slot_sequence_vec in slot_sequences {
        let mut total:f32 = 0.0;
        // let slot_sequence:Vec<SlotType> = slot_sequence.iter().copied().copied().collect(); // wtf rust?
        let mut slot_sequence = ArrayVec::<[SlotType;13]>::new();
        slot_sequence_vec.into_iter().for_each(|x| slot_sequence.push(x));
        let next_slot = slot_sequence.pop().unwrap();
        let mut upper_deficit_now = game.upper_bonus_deficit ;

        let mut head_ev = score_slot(next_slot, game.sorted_dievals); // score slot itself w/o regard to game state adjustments
        let yahtzee_rolled = game.sorted_dievals[0]==game.sorted_dievals[4]; // go on to adjust the raw ev for exogenous game state factors
        if yahtzee_rolled && game.yahtzee_is_wild { 
            if next_slot==SmStraight {head_ev=30}; // extra yahtzees are valid in any lower slot per wildcard rules
            if next_slot==LgStraight {head_ev=40}; 
            if next_slot==FullHouse  {head_ev=25}; 
            head_ev+=100; // extra yahtzee bonus per rules
        }
        if next_slot <= SlotType::Sixes && upper_deficit_now>0 && head_ev>0 { 
            if head_ev >= upper_deficit_now {head_ev+=35}; // add upper bonus when needed total is reached
            upper_deficit_now = upper_deficit_now.saturating_sub(head_ev) ;
        }
        total += head_ev as f32;

        if slot_sequence.len() > 1 { // proceed to also score remaining slots
            let newstate= GameState{
                yahtzee_is_wild: if next_slot==Yahtzee && yahtzee_rolled {true} else {game.yahtzee_is_wild},
                sorted_open_slots: slot_sequence, 
                rolls_remaining: 3,
                upper_bonus_deficit: upper_deficit_now,
                sorted_dievals: game.sorted_dievals, 
            };
            let tail_ev = ev_for_state(&newstate,app); // <---------
            total += tail_ev as f32;
        }
        let total_index:OrderedFloat<f32> = OrderedFloat(total);
        evs.insert(total_index, slot_sequence);
        best_ev = max(best_ev, total_index);
    }

    let best_sequence = evs.get(&best_ev).unwrap();
    let best_slot:SlotType = best_sequence[0];

    (best_slot,best_ev.into_inner())
}

/// returns the best selection of dice and corresponding ev, given slot possibilities and any existing dice and other relevant state 
fn best_dice_ev(s:&GameState, app: &mut AppState) -> (Vec<u8>,f32){ 

    let mut selection_evs:HashMap<OrderedFloat<f32>,Vec<u8>> = HashMap::new();  
    let mut die_combos:Vec<Vec<u8>> = vec![];

    let mut dievals = s.sorted_dievals;
    if s.rolls_remaining==3{ //# we must select all dice on the first roll
        dievals = UNROLLED_DIEVALS;
        die_combos.push(vec![0,1,2,3,4]) ; //all dice
    } else { //  # otherwise we must try all possible combos
        die_combos= die_index_combos();
    }

    for selection in die_combos{ 
        let mut total:f32 = 0.0;
        let outcomes = all_outcomes_for_rolling_n_dice(selection.len() as u8);
        let outcomeslen=outcomes.len();
        for outcome in outcomes{ 
            //###### HOT CODE PATH #######
            let mut newvals=dievals;
            for (i, j) in selection.iter().enumerate() { 
                newvals[*j as usize]=outcome[i];    // TODO performance implications of the cast?
            }
            let mut sorted_newvals = newvals; 
            sorted_newvals.sort_unstable();
            let newstate= GameState{ // TODO slower than individual args?
                yahtzee_is_wild: s.yahtzee_is_wild, 
                sorted_open_slots: s.sorted_open_slots, 
                rolls_remaining: s.rolls_remaining-1,
                upper_bonus_deficit: s.upper_bonus_deficit,
                sorted_dievals: sorted_newvals, 
            };
            let ev =  ev_for_state(&newstate,app);
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

/// returns the additional expected value to come, given relevant game state.
#[cached(key = "GameState", convert = r#"{ *game }"#)] //TODO implement this manually for better control/debugging
fn ev_for_state(game:&GameState, app:&mut AppState) -> f32 { 

    let ev = if game.rolls_remaining == 0 {
        best_slot_ev(game,app).1  // <-----------------
    } else { 
        best_dice_ev(game,app).1  // <-----------------
    };

    {
        app.progress_bar.read().unwrap().println (
            format!("{}\t_\t{:>.0}\t{:?}\t{}\t{}\t{:?}", 
                game.rolls_remaining, ev, game.sorted_dievals, game.upper_bonus_deficit, game.yahtzee_is_wild, game.sorted_open_slots
            )
        );
    }
    // print(log_line,file=log)

    if game.rolls_remaining==3{ // periodically update progress and save
        let mut slotkey:[SlotType;13] = [SlotType::Stub;13];
        slotkey.iter_mut().set_from(game.sorted_open_slots.iter().cloned());
        let is_done = {app.done.contains(&slotkey)} ;
        if ! is_done  {
            app.done.insert(slotkey);
            {app.progress_bar.write().unwrap().inc(1);}
            // if len(done) % 80 == 0 : with open('ev_cache.pkl','wb') as f: pickle.dump(ev_cache,f)
        }
    }
 
    ev 
}