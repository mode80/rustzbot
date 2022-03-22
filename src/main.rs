#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use std::{cmp::max, error::{Error}, fs::{self, File}, time::Duration, ops::{Range, BitAnd, Index, IndexMut}, fmt::Display, u32::MAX};
use itertools::{Itertools, sorted};
use indicatif::ProgressBar;
use rustc_hash::{FxHashSet, FxHashMap};
use tinyvec::*;
use once_cell::sync::Lazy;
use std::io::Write; 
// use cached::proc_macro::cached;

#[macro_use] extern crate serde_derive;
extern crate bincode;

#[cfg(test)] 
#[path = "./tests.rs"]
mod tests;
//-------------------------------------------------------------*/

fn main() -> Result<(), Box<dyn Error>>{
    
    let game = GameState::default();
    let app = & mut AppState::new(&game);

    let EVResult{choice, ev} = best_choice_ev(game, app);
   
    Ok(())
   
}
/*-------------------------------------------------------------*/
type Slots = ArrayVec<[u16;13]>;
type Choice = u16;      // represents EITHER the index of a chosen slot, OR a big-endian bitfield of chosen dice
type DieVal = u16;      // u16 is convient when used with bit math and sets of DieVals, also u16
type Selection = u16;   // " 
type Slot = u16; 
type Score = u16;

#[derive(Debug,Clone,Copy,PartialEq,Serialize,Deserialize,Eq,PartialOrd,Ord,Hash,Default)]
struct DieVals{
    pub data:u16, // 5 dievals (0 to 6) can be encoded in 2 bytes, with each taking 3 bits
}

const FIRST_INDEX_MASK:u16 = 0b111;

impl DieVals {
    fn set(&mut self, index:u16, val:u16) {
        let bitpos = 3*(4-index); // big endian widths of 3 bits per value
        let mask = ! (0b111 << bitpos); // hole maker
        self.data = (self.data & mask) | ((val as u16) << bitpos ); // punch & fill hole
    }
    fn get(&self, index:u16)->u16 {
        (self.data >> ((4-index)*3)) & FIRST_INDEX_MASK
    }
    fn sort(&mut self){ //insertion sort is good for small arrays like this one
        for i in 1..5 {
            let key = self.get(i);
            let mut j = (i as i8) - 1;
            while j >= 0 && self.get(j as u16) > key {
                self.set((j + 1) as u16 , self.get(j as u16) );
                j -= 1;
            }
            self.set((j + 1) as u16, key);
        }
    }
}

impl Display for DieVals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let temp:[u16;5] = self.into(); 
        write!(f,"{:?}",temp) 
    }
}

impl From<[u16; 5]> for DieVals{
    fn from(a: [u16; 5]) -> Self {
        DieVals{data: a[0]<<12 | a[1]<<9 | a[2]<<6 | a[3]<<3 | a[4] }
    }
}

impl From<& DieVals> for [u16; 5]{ 
    fn from(dievals: &DieVals) -> Self {
        let mut temp:[u16;5] = Default::default(); 
        for i in 0_u16..=4 {temp[i as usize] = dievals.get(i)};
        temp
    }
}

impl From<&mut DieVals> for [u16; 5]{ 
    fn from(dievals: &mut DieVals) -> Self {
        <[u16;5]>::from(&*dievals)
    }
}

impl IntoIterator for DieVals{
    type IntoIter=DieValIntoIter;
    type Item = DieVal;

    fn into_iter(self) -> Self::IntoIter {
        DieValIntoIter { data:self, next_idx:0 }
   }

}

struct DieValIntoIter{
    data: DieVals,
    next_idx: u16,
}

impl Iterator for DieValIntoIter {
    type Item = DieVal;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next_idx == 5 {return None};
        let retval = self.data.get(self.next_idx);
        self.next_idx +=1;
        Some(retval)
    }
}

#[derive(Debug,Clone,Copy,Serialize, Deserialize)]
struct EVResult {
    choice: Choice,
    ev: f32
}

#[derive(Debug,Clone,Copy,Default)]
struct Outcome {
    dievals: DieVals,
    mask: DieVals, // stores a pre-made mask for blitting this outcome onto a GameState.DieVals.data u16 later
    arrangements_count: u8,
}

#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Clone, Copy, Serialize, Deserialize)]
struct GameState{
    sorted_dievals:DieVals, 
    rolls_remaining:u8, 
    upper_bonus_deficit:u16, 
    yahtzee_is_wild:bool,
    sorted_open_slots:Slots, 
}
impl Default for GameState{
    fn default() -> Self {
        Self { sorted_dievals: Default::default(), rolls_remaining: 3, upper_bonus_deficit: 63, 
            yahtzee_is_wild: false, sorted_open_slots: [1,2,3,4,5,6,7,8,9,10,11,12,13].into(),
        }
    }
}

// #[derive(Debug,Clone,Copy,Serialize,Deserialize,PartialEq)]
// enum Choice{
//     Slot(u8),
//     Dice(DiceBits)
// }

struct AppState{
    progress_bar:ProgressBar, 
    done:FxHashSet<Slots>, 
    ev_cache:FxHashMap<GameState,EVResult>,
    checkpoint: Duration,
}
impl AppState{
    fn new(game: &GameState) -> Self{
        let slot_count=game.sorted_open_slots.len() as u8;
        let combo_count = (1..=slot_count).map(|r| n_take_r(slot_count, r ,false,false) as u64 ).sum() ;
        let init_capacity = combo_count as usize * 252 * 64; // * 2 * 2; // roughly: slotcombos * diecombos * deficits * wilds * rolls
        let cachemap = if let Ok(bytes) = fs::read("ev_cache") { 
            ::bincode::deserialize(&bytes).unwrap() 
        } else {
            FxHashMap::with_capacity_and_hasher(init_capacity,Default::default())
        };
        Self{   progress_bar : ProgressBar::new(combo_count), 
                done : Default::default() ,  
                ev_cache : cachemap,
                checkpoint: Duration::new(0,0),
        }
    }
}

const STUB:u16=0; const ACES:u16=1; const TWOS:u16=2; const THREES:u16=3; const FOURS:u16=4; const FIVES:u16=5; const SIXES:u16=6;
const THREE_OF_A_KIND:u16=7; const FOUR_OF_A_KIND:u16=8; const FULL_HOUSE:u16=9; const SM_STRAIGHT:u16=10; const LG_STRAIGHT:u16=11; 
const YAHTZEE:u16=12; const CHANCE:u16=13; 
 
#[allow(clippy::unusual_byte_groupings)] // each group of 3 bits encodes a dieval from 0 to 6
const UNROLLED_DIEVALS:DieVals = DieVals{data:0};  //just use DieVals::default()
const INIT_DEFICIT:u16 = 63;

const SCORE_FNS:[fn(sorted_dievals:DieVals)->Score;14] = [
    score_aces, // duplicate placeholder so indices align more intuitively with categories 
    score_aces, score_twos, score_threes, score_fours, score_fives, score_sixes, 
    score_3ofakind, score_4ofakind, score_fullhouse, score_sm_str8, score_lg_str8, score_yahtzee, score_chance, 
];

static SELECTION_RANGES:Lazy<[Range<usize>;32]> = Lazy::new(selection_ranges); 

static SELECTION_OUTCOMES:Lazy<[Outcome;1683]> = Lazy::new(all_selection_outcomes); 
// [(), (0,), (0, 1), (0, 1, 2), (0, 1, 2, 3), (0, 1, 2, 3, 4), (0, 1, 2, 4), (0, 1, 3), (0, 1, 3, 4), 
// (0, 1, 4), (0, 2), (0, 2, 3), (0, 2, 3, 4), (0, 2, 4), (0, 3), (0, 3, 4), (0, 4), (1,), (1, 2), (1, 2, 3), (1, 2, 3, 4), 
// (1, 2, 4), (1, 3), (1, 3, 4), (1, 4), (2,), (2, 3), (2, 3, 4), (2, 4), (3,), (3, 4), (4,)]

/*-------------------------------------------------------------*/

/// rudimentary factorial suitable for our purposes here.. handles up to fact(34) */
fn fact(n: u8) -> u128{
    let big_n = n as u128;
    if n<=1 {1} else { (big_n)*fact(n-1) }
}

fn distinct_arrangements_for(dieval_vec:Vec<u16>)->u8{
    let counts = dieval_vec.iter().counts();
    let mut divisor:usize=1;
    let mut non_zero_dievals=0_u8;
    for count in counts { 
        if *count.0 != 0 { 
            divisor *= fact(count.1 as u8) as usize ; 
            non_zero_dievals += count.1 as u8;
        }
    } 
    (fact(non_zero_dievals) as f64 / divisor as f64) as u8
}

/// count of arrangements that can be formed from r selections, chosen from n items, 
/// where order DOES or DOESNT matter, and WITH or WITHOUT replacement, as specified
fn n_take_r(n:u8, r:u8, ordered:bool, with_replacement:bool)->u128{

    if !ordered { // we're counting "combinations" where order doesn't matter, so there are less of these 
        if with_replacement {
            fact(n+r-1) / (fact(r)*fact(n-1))
        } else { // no replacement
            fact(n) / (fact(r)*fact(n-r)) 
        }
    } else { // is ordered
        if with_replacement {
            (n as u128).pow(r as u32)
        } else { // no replacement
            fact(n) / fact(n-r)
        }
    }
}

fn save_periodically(app:&mut AppState, every_n_secs:u64){
    let current_duration = app.progress_bar.elapsed();
    let last_duration = app.checkpoint;
    if current_duration - last_duration >= Duration::new(every_n_secs,0) { 
        app.checkpoint = current_duration;
        save_cache(app);
    }
}

fn save_cache(app:&AppState){
    let evs = &app.ev_cache; 
    let mut f = &File::create("ev_cache").unwrap();
    let bytes = bincode::serialize(evs).unwrap();
    f.write_all(&bytes).unwrap();
}
 
fn console_log(game:&GameState, app:&AppState, choice:Choice, ev:f32 ){
    app.progress_bar.println (
        format!("{:>}\t{}\t{:>4}\t{:>4.2}\t{:?}\t{:?}\t{:?}", 
            game.rolls_remaining, game.yahtzee_is_wild, game.upper_bonus_deficit, ev, game.sorted_dievals, game.sorted_open_slots, choice 
        )
    );
}

struct SlotPermutations{
    a:Slots,
    c:[usize;13],// c is an encoding of the stack state. c[k] encodes the for-loop counter for when generate(k - 1, A) is called
    i:usize,// i acts similarly to a stack pointer
}
impl SlotPermutations{
    fn new(a:Slots) -> Self{
        let c= [0;13];
        let i = 255;
        Self{ a, c, i}
    }
}
impl Iterator for SlotPermutations{
    type Item = Slots;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i==255 { self.i=0; return Some(self.a); } // first run
        if self.i == self.a.len()  {return None}; // last run
        if self.c[self.i] < self.i { 
            if self.i % 2 == 0 { // even 
                (self.a[self.i], self.a[0]) = (self.a[0], self.a[self.i]); //swap 
            } else { // odd
                (self.a[self.c[self.i]], self.a[self.i]) = (self.a[self.i], self.a[self.c[self.i]]); //swap
            } 
            self.c[self.i] += 1;// Swap has occurred ending the "for-loop". Simulate the increment of the for-loop counter
            self.i = 0;// Simulate recursive call reaching the base case by bringing the pointer to the base case analog in the array
            Some(self.a)
        } else { // Calling generate(i+1, A) has ended as the for-loop terminated. Reset the state and simulate popping the stack by incrementing the pointer.
            self.c[self.i] = 0;
            self.i += 1;
            self.next()
        } 
    }
}

/*-------------------------------------------------------------*/
//the set of roll outcomes for every possible 5-die selection, where '0' represents an unselected die
fn all_selection_outcomes() ->[Outcome;1683]  { 
    let mut retval:[Outcome;1683] = [Default::default();1683];
    let mut outcome = Outcome::default();
    let mut i=0;
    let mut mask:DieVals; 
    for sel_idxs in die_index_combos(){
        outcome.dievals = Default::default();
        for dievals_vec in [1,2,3,4,5,6_u16].into_iter().combinations_with_replacement(sel_idxs.len()){ 
            outcome.mask = [0b111,0b111,0b111,0b111,0b111].into();
            for (j, &val ) in dievals_vec.iter().enumerate() { 
                let idx = 4-sel_idxs[j] as u16; // count down the indexes so it maps naturally to a big-endian bitfield 
                outcome.dievals.set(idx,val) ; 
                outcome.mask.set(idx,0);
            }
            outcome.arrangements_count = distinct_arrangements_for(dievals_vec);
            retval[i]=outcome;
            i+=1;
        }
    } 
    retval
}

/// this generates the ranges that correspond to the outcomes for a given selection, within the set of all outcomes above
fn selection_ranges() ->[Range<usize>;32]  { 
    let mut sel_ranges:[Range<usize>;32] = Default::default();
    let mut s = 0;
    for (i,combo) in die_index_combos().into_iter().enumerate(){
        let count = n_take_r(6, combo.len() as u8, false, true) ;
        sel_ranges[i] = s..(s+count as usize);
        s += count as usize; 
    }
    sel_ranges
}

/// the set of all ways to roll different dice, as represented by a collection of index arrays
#[allow(clippy::eval_order_dependence)]
fn die_index_combos() ->[ArrayVec<[u16;5]>;32]  { 
    let mut them = [ArrayVec::<[u16;5]>::new() ;32]; // init dice arrray 
    let mut i=0; 
    for n in 1..=5 {
        for combo in (0..=4).combinations(n){ 
            them[i]= { let mut it=ArrayVec::<[u16;5]>::new(); it.extend_from_slice(&combo); i+=1; it} 
        } 
    }
    // them.sort_unstable();
    them
}

fn score_upperbox(boxnum:Slot, sorted_dievals:DieVals)->Score{
   sorted_dievals.into_iter().filter(|x| *x==boxnum).sum()
}

fn score_n_of_a_kind(n:u8,sorted_dievals:DieVals)->Score{
    let mut inarow=1; let mut maxinarow=1; let mut lastval=100; let mut sum=0; 
    for x in sorted_dievals {
        if x==lastval && x!=0 {inarow +=1} else {inarow=1}
        maxinarow = max(inarow,maxinarow);
        lastval = x;
        sum+=x;
    }
    if maxinarow>=n {
        sum
    } else {0}
}


fn straight_len(sorted_dievals:DieVals)->u8 {
    let mut inarow=1; 
    let mut maxinarow=1; 
    let mut lastval=254; // stub
    for x in sorted_dievals {
        if x==lastval+1 && x!=0 {inarow+=1}
        else if x!=lastval {inarow=1};
        maxinarow = max(inarow,maxinarow);
        lastval = x;
    } 
    maxinarow 
}

fn score_aces(sorted_dievals:       DieVals)->Score{ score_upperbox(1,sorted_dievals) }
fn score_twos(sorted_dievals:       DieVals)->Score{ score_upperbox(2,sorted_dievals) }
fn score_threes(sorted_dievals:     DieVals)->Score{ score_upperbox(3,sorted_dievals) }
fn score_fours(sorted_dievals:      DieVals)->Score{ score_upperbox(4,sorted_dievals) }
fn score_fives(sorted_dievals:      DieVals)->Score{ score_upperbox(5,sorted_dievals) }
fn score_sixes(sorted_dievals:      DieVals)->Score{ score_upperbox(6,sorted_dievals) }

fn score_3ofakind(sorted_dievals:   DieVals)->Score{ score_n_of_a_kind(3,sorted_dievals) }
fn score_4ofakind(sorted_dievals:   DieVals)->Score{ score_n_of_a_kind(4,sorted_dievals) }
fn score_sm_str8(sorted_dievals:    DieVals)->Score{ if straight_len(sorted_dievals) >=4 {30} else {0} }
fn score_lg_str8(sorted_dievals:    DieVals)->Score{ if straight_len(sorted_dievals) >=5 {40} else {0} }

// The official rule is that a Full House is "three of one number and two of another"
fn score_fullhouse(sorted_dievals:DieVals) -> Score { 
    let counts_map = sorted_dievals.into_iter().counts();
    let mut counts = counts_map.values().collect_vec(); 
    counts.sort_unstable();
    if  (counts.len()==2) && 
        (*counts[0]==2 && *counts[1]==3) &&
        (*counts[0]!=0 && *counts[1]!=0)
    {25} else {0}
}

fn score_chance(sorted_dievals:DieVals)->Score { sorted_dievals.into_iter().sum()  }
fn score_yahtzee(sorted_dievals:DieVals)->Score { 
    let deduped=sorted_dievals.into_iter().dedup().collect_vec(); //TODO banish use of vecs for less allocating
    if deduped.len()==1 && sorted_dievals.get(0)!=0 {50} else {0} 
}

/// reports the score for a set of dice in a given slot w/o regard for exogenous gamestate (bonuses, yahtzee wildcards etc)
fn score_slot(slot:Slot, sorted_dievals:DieVals)->Score{
    SCORE_FNS[slot as usize](sorted_dievals) 
}
/*-------------------------------------------------------------*/

/// returns the best slot and corresponding ev for final dice, given the slot possibilities and other relevant state 
fn best_slot_ev(game:GameState, app: &mut AppState) -> EVResult  {

    // TODO consider slot_sequences.chunk(___) + multi-threading
    let mut best_ev = 0.0; 
    let mut best_slot=STUB; 

    for mut slot_sequence in SlotPermutations::new(game.sorted_open_slots) {

        // LEAF CALCS 
            // prep vars
                let mut tail_ev = 0.0;
                let top_slot = slot_sequence.pop().unwrap(); //TODO try not mutating this
                let mut _choice = top_slot;
                let mut upper_deficit_now = game.upper_bonus_deficit ;
                let mut yahtzee_wild_now:bool = game.yahtzee_is_wild;

            // score slot itself w/o regard to game state 
                let mut head_ev = score_slot(top_slot, game.sorted_dievals); 

            // add upper bonus when needed total is reached
                if top_slot <= SIXES && upper_deficit_now>0 && head_ev>0 { 
                    if head_ev >= upper_deficit_now {head_ev += 35}; 
                    upper_deficit_now = upper_deficit_now.saturating_sub(head_ev) ;
                } 

            // special handling of "extra yahtzees" 
                let yahtzee_rolled = game.sorted_dievals.get(0)==game.sorted_dievals.get(4); 
                if yahtzee_rolled && game.yahtzee_is_wild { // extra yahtzee situation
                    if top_slot==SM_STRAIGHT {head_ev=30}; // extra yahtzees are valid in any lower slot, per wildcard rules
                    if top_slot==LG_STRAIGHT {head_ev=40}; 
                    if top_slot==FULL_HOUSE  {head_ev=25}; 
                    head_ev+=100; // extra yahtzee bonus per rules
                }
                if top_slot==YAHTZEE && yahtzee_rolled {yahtzee_wild_now = true} ;


        if ! slot_sequence.is_empty() { // proceed to include all the ev of remaining slots in this slot_sequence

            //prune unneeded state duplication when there's no chance of reaching upper bonus
                let mut best_deficit = upper_deficit_now;
                for upper_slot in slot_sequence{ 
                    if upper_slot > 6 || best_deficit==0 {break};
                    best_deficit = best_deficit.saturating_sub(upper_slot*5);
                } // an impossible upper bonus is same ev as the "full deficit" scenario, where the ev may already be cached
                if best_deficit > 0 {upper_deficit_now=63}; 

            // we'll permutate and find max ev on the inside down below, but we'll use this sorted sequence as the key when we cache the max 
                slot_sequence.sort_unstable(); 

            // gather the collective ev for the remaining slots recursively
                EVResult{choice:_choice, ev:tail_ev} = best_choice_ev( GameState{
                    yahtzee_is_wild: yahtzee_wild_now,
                    sorted_open_slots: slot_sequence, 
                    rolls_remaining: 3,
                    upper_bonus_deficit: upper_deficit_now,
                    sorted_dievals: game.sorted_dievals, 
                },app);
        }

        let ev = tail_ev + head_ev as f32 ; 
        if ev >= best_ev { best_ev = ev; best_slot = top_slot ; }

    } // end for slot_sequence...

    EVResult{choice:best_slot, ev:best_ev}
}

/// returns the best selection of dice and corresponding ev, given slots left, existing dice, and other relevant state 
fn best_dice_ev(game:GameState, app: &mut AppState) -> EVResult { 

    let mut best_selection:u16 = 0b11111; // default selection is "all dice"
    let mut best_ev = 0.0; 
    if game.rolls_remaining==3 {// special case .. we always roll all dice on initial roll
        best_ev = avg_ev_for_selection(game,app,best_selection);
    } else { // iterate over all the possible ways to select dice and take the best outcome 
        for selection in 0_u16..=31 { // each selection is a u8 encoded bitfield
            let avg_ev = avg_ev_for_selection(game,app,selection);
            if avg_ev > best_ev {
                best_ev = avg_ev; 
                best_selection = selection; 
            }
        }
    }
    EVResult{choice:best_selection, ev:best_ev}
}

/// returns the average of all the expected values for rolling a selection of dice, given the game and app state
/// "selection" is the set of dice to roll, as represented their indexes in a 5-length array
#[inline(always)] // ~6% speedup 
fn avg_ev_for_selection(game:GameState, app: &mut AppState, selection:Selection) -> f32 {
    let mut total_ev = 0.0;
    let mut newvals:DieVals; 

    let range = SELECTION_RANGES[selection as usize].clone(); // selection bitfield also acts as index into the cached ranges for corresponding outcomes 
    let mut outcomes_count:usize = 0; 
    for outcome in SELECTION_OUTCOMES[range].iter() { 
        //###### HOT CODE PATH #######
        newvals = Default::default(); 
        newvals.data = (game.sorted_dievals.data & outcome.mask.data) | outcome.dievals.data; //gives result after rolling selected dice. faster than looping looping
        newvals.sort();
        let EVResult{choice, ev} = best_choice_ev( GameState{ 
            yahtzee_is_wild: game.yahtzee_is_wild, 
            sorted_open_slots: game.sorted_open_slots, 
            rolls_remaining: game.rolls_remaining-1,
            upper_bonus_deficit: game.upper_bonus_deficit,
            sorted_dievals: newvals, 
        }, app);
        outcomes_count += outcome.arrangements_count as usize; // we loop through "combos" but we must sum all "perumtations"
        let added_ev = ev * outcome.arrangements_count as f32; // each combo's ev is weighted by its count of distinct arrangements
        total_ev += added_ev;
        //############################
        // eprintln!("{} {} {} {} {} {}", game.rolls_remaining, outcome.dievals, outcome.arrangements_count, newvals, ev, added_ev); 
    }
    total_ev/outcomes_count as f32 
}


/// returns the best game Choice along with its expected value, given relevant game state.
// #[cached(key = "GameState", convert = r#"{ game }"#)] 
fn best_choice_ev(game:GameState,app: &mut AppState) -> EVResult  { 

    if let Some(result) = app.ev_cache.get(&game) { return *result}; // return cached result if we have one 
    // cache contention here during constant cache writing effectively caps us to single threaded speeds
    // TODO consider periodically "freezing" chuncks of completed cache into read-only state for better multithreading

    let result = if game.rolls_remaining == 0 { //TODO figure out a non-recursive version of this (better for multi-threading)?
        best_slot_ev(game,app)  // <-----------------
    } else { 
        best_dice_ev(game,app)  // <-----------------
    };

    // console_log(&game,app,result.0,result.1);

    if game.rolls_remaining==0 { // periodically update progress and save
        let e = {app.done.contains(&game.sorted_open_slots)} ;
        if ! e  {
            app.done.insert(game.sorted_open_slots);
            app.progress_bar.inc(1);
            console_log(&game,app, result.choice, result.ev);
            save_periodically(app,600) ;
        }
    }
    
    app.ev_cache.insert(game, result);
    result 
}

