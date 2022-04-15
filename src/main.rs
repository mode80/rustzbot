#![allow(dead_code)] #![allow(unused_imports)] #![allow(unused_variables)]
#![allow(clippy::needless_range_loop)] #![allow(clippy::unusual_byte_groupings)] 

use std::{thread::{self, spawn, sleep}, sync::{Mutex, Arc, mpsc}, ops::Index, cmp::Ordering, time::Instant};
use std::{cmp::{max, min}, fs::{self, File}, time::Duration, ops::Range, fmt::Display, panic};
use itertools::{Itertools, iproduct, repeat_n};
use indicatif::{ProgressBar, ProgressStyle, ProgressFinish};
use rustc_hash::{FxHashMap, FxHashSet};
use once_cell::sync::Lazy;
use std::io::Write; 
use rayon::prelude::*;

#[macro_use] extern crate serde_derive;
extern crate bincode;

#[cfg(test)] 
#[path = "./tests.rs"]
mod tests;

/*------------------------------------------------------------
MAIN
-------------------------------------------------------------*/
fn main() {
    
    let game = GameState::default();
    let app = & mut AppState::new(&game);

    let ChoiceEV{choice, ev} = best_choice_ev(game, app);
}

/*-------------------------------------------------------------
TYPE ALIASES
-------------------------------------------------------------*/
type Choice = u8; // represents EITHER the index of a chosen slot, OR a DieSet selection (below)
type DieSet = u8; // encodes a selection of which dice to roll where 0b11111 means "all five dice" and 0b00101 means "first and third"
type DieVal = u8; // a single die value 0 to 6 where 0 means "unselected"
type Slot   = u8; // a single slot with values ranging from ACES to CHANCE 
type Score  = u8;


/*-------------------------------------------------------------
Slots
-------------------------------------------------------------*/

#[derive(Debug,Clone,Copy,PartialEq,Serialize,Deserialize,Eq,PartialOrd,Ord,Hash,Default)]

struct Slots{
    pub data:u64, // 13 Slot values of between 1 and 13 can be encoded within these 8 bytes, each taking 4 bits
    pub len:u8,
}

impl Slots {

    fn set(&mut self, index:u8, val:Slot) { 
        debug_assert!(index < self.len); 
        debug_assert!(index < 13); 
        let bitpos = 4*index; // widths of 4 bits per value 
        let mask = ! (0b1111 << bitpos); // hole maker
        self.data = (self.data & mask) | ((val as u64) << bitpos ); // punch & fill hole
    }

    fn get(&self, index:u8)->Slot{
        ((self.data >> (index*4)) & 0b1111) as Slot 
    }

    fn push(&mut self, val:Slot){
        self.len +=1;
        self.set(self.len-1,val);
    }

    fn truncate(&mut self, len:u8) {
        let mask = (2_u64).pow(len as u32 * 4)-1;
        self.data &= mask;
        self.len=len;
    }

    fn truncated(self, len:u8) -> Self {
        let mut self_copy = self;
        self_copy.truncate(len);
        self_copy
    }

    fn subset(self, start_idx: u8, max_len:u8) -> Self{
        let mut self_copy = self;
        self_copy.data >>= start_idx*4;
        let len = min(max_len, self.len-start_idx);
        self_copy.truncate(len);
        self_copy
    }

    fn sort(&mut self){ 
        for i in 1..self.len { // "insertion sort" is good for small arrays like this one
            let key = self.get(i);
            let mut j = (i as i8) - 1;
            while j >= 0 && self.get(j as u8) > key {
                self.set((j + 1) as u8 , self.get(j as u8) );
                j -= 1;
            }
            self.set((j + 1) as u8, key);
        }
    }

    fn pop(&mut self) -> Slot {
        let retval = self.get(self.len-1);
        self.set(self.len-1,0); 
        self.len -=1; 
        retval
    }

    fn permutations (self) -> SlotPermutations{
        SlotPermutations::new(self,0,self.len)
    }

    fn permutations_within (self,start:u8,len:u8) -> SlotPermutations{
        SlotPermutations::new(self,start,len)
    }

    // // given this set of slots, what set of slots have previously been played? (ie the inverse set)
    // fn previously_played (self) -> Self{
    //     let mut ret:Self = Default::default();
    //     let mut i=0;
    //     for s in ACES..CHANCE {
    //         if !self.into_iter().contains(&s) {ret.set(i,s); i+=1;}
    //     };
    //     ret
    // }

    fn missing_upper_slots(self) -> Self{
        let upper_slots= FxHashSet::<u8>::from_iter(self.into_iter().filter(|&x|x<=SIXES));
        let mut retval:Slots = Default::default();
        for s in ACES..=SIXES { if !upper_slots.contains(&s) {retval.push(s)}; }
        retval
    }
 
    /// returns the unique "upper bonus totals" shortfalls that could have occurred from the missing upper slots 
    fn unique_upper_deficits(self) -> Vec<u8> { //impl Iterator<Item=u8> {  // TODO implement without allocating?
        let mut unique_totals:FxHashSet<u8> = Default::default();
        // these are all the possible score entries for each upper slot
        const UPPER_SCORES:[[u8;6];7] = [ 
            [0,0,0,0,0,0],      // STUB
            [0,1,2,3,4,5],      // ACES
            [0,2,4,6,8,10],     // TWOS
            [0,3,6,9,12,15],    // THREES 
            [0,4,8,12,16,20],   // FOURS
            [0,5,10,15,20,25],  // FIVES
            [0,6,12,18,24,30],  // SIXES
        ];
        // only upper slots could have contributed to the upper total 
        let slot_idxs = self.into_iter().filter(|&x|x<=SIXES).map(|x| x as usize).collect_vec();
        let score_idx_perms= repeat_n(0..=5, slot_idxs.len()).multi_cartesian_product();
        // for every permutation of entry indexes
        for score_idxs in score_idx_perms {
            // covert the list of entry indecis to a list of entry -scores-, then total them
            let tot = slot_idxs.iter().zip(score_idxs).map(|(i,ii)| UPPER_SCORES[*i][ii]).sum();
            // add the total to the set of unique totals 
            unique_totals.insert(tot);
        }
        unique_totals.into_iter().map(|x|63_u8.saturating_sub(x)).unique().collect_vec()
    }

}

impl Display for Slots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let temp:[Slot;13] = self.into(); 
        let mut temp_vec = vec![0;13];
        temp_vec.copy_from_slice(&temp);
        // write!(f,"{:?}",temp.into_iter().filter(|x|*x!=0).collect_vec()) 
        write!(f,"{:?}",temp.into_iter().take(self.len as usize).collect_vec()) 
    }
}

impl <const N:usize> From<[Slot; N]> for Slots{
    fn from(a: [Slot; N]) -> Self {
        if a.len() as usize > 13 { panic!(); }
        let mut retval = Slots{ len:a.len() as u8, data:Default::default()};
        for i in 0..N { retval.set(i as u8, a[i as usize]); }
        retval 
    }
}
impl <const N:usize>  From<&Slots> for [Slot; N]{ 
    fn from(slots: &Slots) -> Self {
        if slots.len as usize > N { panic!(); }
        let mut retval:[Slot;N] = [Slot::default(); N]; 
        for i in 0..N {retval[i] = slots.get(i as u8)};
        retval
    }
}
impl <const N:usize>  From<&mut Slots> for [Slot; N]{ 
    fn from(slots: &mut Slots) -> Self {
        <[Slot;N]>::from(&*slots) // the &* here copies the mutable ref to a ref 
    }
}

impl IntoIterator for Slots{
    type IntoIter=SlotIntoIter;
    type Item = Slot;

    fn into_iter(self) -> Self::IntoIter {
        SlotIntoIter { slots:self, next_idx:0 }
    }
}

struct SlotIntoIter{
    slots: Slots,
    next_idx: u8,
}

impl Iterator for SlotIntoIter {
    type Item = Slot ;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next_idx == self.slots.len {return None};
        let retval = self.slots.get(self.next_idx);
        self.next_idx +=1;
        Some(retval)
    }
}

/*-------------------------------------------------------------
Slot Permutations
-------------------------------------------------------------*/

struct SlotPermutations{
    slots:Slots,
    c:[usize;13],   // c is an encoding of the stack state. c[k] encodes the for-loop counter for when generate(k - 1, A) is called
    i:usize,        // i acts similarly to a stack pointer
    start:u8,       // start index 
    end:u8,         // end index exclusive 
}
impl SlotPermutations{
    fn new(slots:Slots, start:u8, len:u8) -> Self{
        // let length_mask:u64 = 2_u64.pow(4 * k as u32)-1; :Slots{data:slots.data & length_mask, len:k}
        Self{ slots, c:[start as usize;13], i:255, end:min(start+len,slots.len), start}
    }
}
impl Iterator for SlotPermutations{
    type Item = Slots;

    fn next(&mut self) -> Option<Self::Item> { //Heap's algorithm for generating permutations, modified for permuting interior ranges
        if self.i==255 { self.i=self.start as usize; return Some(self.slots); } // first run
        if self.i == self.end as usize {return None}; // last run
        if self.c[self.i] < self.i { 
            if (self.i + self.start as usize) & 1 == 0 { // odd iteration
                let temp = self.slots.get(self.i as u8); // prep to swap
                self.slots.set(self.i as u8, self.slots.get(self.start));
                self.slots.set(self.start, temp);
            } else { // even iteration 
                let temp = self.slots.get(self.c[self.i] as u8); //prep to swap
                self.slots.set(self.c[self.i] as u8, self.slots.get(self.i as u8));
                self.slots.set(self.i as u8, temp);
            } 
            self.c[self.i] += 1;// Swap has occurred ending the "for-loop". Simulate the increment of the for-loop counter
            self.i = self.start as usize;// Simulate recursive call reaching the base case by bringing the pointer to the base case analog in the array
            Some(self.slots)
        } else { // Calling generate(i+1, A) has ended as the for-loop terminated. Reset the state and simulate popping the stack by incrementing the pointer.
            self.c[self.i] = self.start as usize;
            self.i += 1;
            self.next()
        } 
    }
}


/*-------------------------------------------------------------
DieVals
-------------------------------------------------------------*/

#[derive(Debug,Clone,Copy,PartialEq,Serialize,Deserialize,Eq,PartialOrd,Ord,Hash,Default)]

struct DieVals{
    pub data:u16, // 5 dievals (0 to 6) can be encoded in 2 bytes total, each taking 3 bits
}

impl DieVals {

    fn set(&mut self, index:u8, val:DieVal) { 
        let bitpos = 3*index; // widths of 3 bits per value
        let mask = ! (0b111 << bitpos); // hole maker
        self.data = (self.data & mask) | ((val as u16) << bitpos ); // punch & fill hole
    }

    fn get(&self, index:u8)->DieVal{
        ((self.data >> (index*3)) & 0b111) as DieVal
    }

    fn sort(&mut self){ //insertion sort is good for small arrays like this one
        for i in 1..5 {
            let key = self.get(i);
            let mut j = (i as i8) - 1;
            while j >= 0 && self.get(j as u8) > key {
                self.set((j + 1) as u8 , self.get(j as u8) );
                j -= 1;
            }
            self.set((j + 1) as u8, key);
        }
    }
}

impl Display for DieVals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,"{}{}{}{}{}",self.get(0), self.get(1),self.get(2),self.get(3),self.get(4)) 
    }
}

impl From<[DieVal; 5]> for DieVals{
    fn from(a: [DieVal; 5]) -> Self {
        DieVals{data: (a[4] as u16) << 12 | (a[3] as u16) <<9 | (a[2] as u16) <<6 | (a[1] as u16) <<3 | (a[0] as u16)}
    }
}

impl From<& DieVals> for [DieVal; 5]{ 
    fn from(dievals: &DieVals) -> Self {
        let mut temp:[DieVal;5] = Default::default(); 
        for i in 0_u8..=4 {temp[i as usize] = dievals.get(i)};
        temp
    }
}

impl From<DieVals> for [DieVal; 5]{ 
    fn from(dievals: DieVals) -> Self {
        <[DieVal;5]>::from(&dievals)
    }
}

impl From<&mut DieVals> for [DieVal; 5]{ 
    fn from(dievals: &mut DieVals) -> Self {
        <[DieVal;5]>::from(&*dievals)
    }
}

impl IntoIterator for DieVals{
    type IntoIter=DieValsIntoIter;
    type Item = DieVal;

    fn into_iter(self) -> Self::IntoIter {
        DieValsIntoIter { data:self, next_idx:0 }
   }

}

struct DieValsIntoIter{
    data: DieVals,
    next_idx: u8,
}

impl Iterator for DieValsIntoIter {
    type Item = DieVal;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next_idx == 5 {return None};
        let retval = self.data.get(self.next_idx);
        self.next_idx +=1;
        Some(retval)
    }
}

/*-------------------------------------------------------------
ChoiceEV
-------------------------------------------------------------*/
#[derive(Debug,Clone,Copy,Serialize, Deserialize, Default)]
struct ChoiceEV {
    choice: Choice,
    ev: f32
}


/*-------------------------------------------------------------
Outcome
-------------------------------------------------------------*/
#[derive(Debug,Clone,Copy,Default)]
struct Outcome {
    dievals: DieVals,
    mask: DieVals, // stores a pre-made mask for blitting this outcome onto a GameState.DieVals.data u16 later
    arrangements: u8, // how many indistinguisable ways can these dievals be arranged (ie swapping identical dievals)
}

/*-------------------------------------------------------------
GameState
-------------------------------------------------------------*/
#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Clone, Copy, Serialize, Deserialize)]
struct GameState{
    sorted_dievals:DieVals, 
    rolls_remaining:u8, 
    upper_bonus_deficit:u8, 
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

/*-------------------------------------------------------------
AppState
-------------------------------------------------------------*/
struct AppState{
    progress_bar:ProgressBar, 
    ev_cache:FxHashMap<GameState,ChoiceEV>,
    checkpoint: Duration,
    // done:FxHashSet<Slots>,
}
impl AppState{
    fn new(game: &GameState) -> Self{
        let slot_count=game.sorted_open_slots.len as usize;
        let slot_combos:u64 = (1..=slot_count).map(|r| n_take_r(slot_count, r ,false,false) as u64 ).sum() ;
        let slot_perms:u64 = (1..=slot_count).map(|r| n_take_r(slot_count, r ,true,false) as u64 ).sum() ;
        let pb = ProgressBar::new(slot_perms); 
        pb.set_style(ProgressStyle::default_bar()
            .template("{prefix} {wide_bar} {percent}% {pos:>4}/{len:4} {elapsed:>}/{duration} ETA:{eta}")
            .on_finish(ProgressFinish::Abandon)
        );
        let init_capacity = slot_combos as usize * 252 * 64; // * 2 * 2; // roughly: slotcombos * diecombos * deficits * wilds * rolls
        let cachemap = if let Ok(bytes) = fs::read("ev_cache") { 
            ::bincode::deserialize(&bytes).unwrap() 
        } else {
            FxHashMap::with_capacity_and_hasher(init_capacity,Default::default())
        };
        let cache_keys:Vec<&GameState> = cachemap.keys().into_iter().collect_vec();
        let former_ticks:u64 = cache_keys.into_iter().filter(|x|x.rolls_remaining ==0).map(|x|FACT[x.sorted_open_slots.len as usize] ).sum();
        pb.inc(former_ticks);
        Self{   progress_bar : pb, 
                ev_cache : cachemap,
                checkpoint: Duration::new(0,0),
                // done: FxHashSet::default(), 
        }
    }

    fn save_periodically(&mut self , every_n_secs:u64){
        let current_duration = self.progress_bar.elapsed();
        let last_duration = self.checkpoint;
        if current_duration - last_duration >= Duration::new(every_n_secs,0) { 
            self.checkpoint = current_duration;
            self.save_cache();
        }
    }

    fn save_cache(&self){
        let evs = &self.ev_cache; 
        let mut f = &File::create("ev_cache").unwrap();
        let bytes = bincode::serialize(evs).unwrap();
        f.write_all(&bytes).unwrap();
    }

}

/*-------------------------------------------------------------
CONSTS
-------------------------------------------------------------*/

const STUB:Slot=0; const ACES:Slot=1; const TWOS:Slot=2; const THREES:Slot=3; const FOURS:Slot=4; const FIVES:Slot=5; const SIXES:Slot=6;
const THREE_OF_A_KIND:Slot=7; const FOUR_OF_A_KIND:Slot=8; const FULL_HOUSE:Slot=9; const SM_STRAIGHT:Slot=10; const LG_STRAIGHT:Slot=11; 
const YAHTZEE:Slot=12; const CHANCE:Slot=13; 
 
const INIT_DEFICIT:u8 = 63;

const SCORE_FNS:[fn(sorted_dievals:DieVals)->Score;14] = [
    score_aces, // duplicate placeholder so indices align more intuitively with categories 
    score_aces, score_twos, score_threes, score_fours, score_fives, score_sixes, 
    score_3ofakind, score_4ofakind, score_fullhouse, score_sm_str8, score_lg_str8, score_yahtzee, score_chance, 
];

static SELECTION_RANGES:Lazy<[Range<usize>;32]> = Lazy::new(selection_ranges); 
static SELECTION_OUTCOMES:Lazy<[Outcome;1683]> = Lazy::new(all_selection_outcomes); 
static FACT:Lazy<[u64;21]> = Lazy::new(||{let mut a:[u64;21]=[0;21]; for i in 0..=20 {a[i]=fact(i as u8);} a});  // cached factorials
static CORES:Lazy<usize> = Lazy::new(num_cpus::get);


/*-------------------------------------------------------------
INITIALIZERS
-------------------------------------------------------------*/

/// this generates the ranges that correspond to the outcomes, within the set of all outcomes, indexed by a give selection 
fn selection_ranges() ->[Range<usize>;32]  { 
    let mut sel_ranges:[Range<usize>;32] = Default::default();
    let mut s = 0;
    sel_ranges[0] = 0..1;
    for (i,combo) in die_index_combos().into_iter().enumerate(){
        let count = n_take_r(6, combo.len(), false, true) ;
        sel_ranges[i] = s..(s+count as usize);
        s += count as usize; 
    }
    sel_ranges
}

//the set of roll outcomes for every possible 5-die selection, where '0' represents an unselected die
fn all_selection_outcomes() ->[Outcome;1683]  { 
    let mut retval:[Outcome;1683] = [Default::default();1683];
    let mut outcome = Outcome::default();
    let mut i=0;
    for combo in die_index_combos(){
        outcome.dievals = Default::default();
        for dievals_vec in [1,2,3,4,5,6_u8].into_iter().combinations_with_replacement(combo.len()){ 
            outcome.mask = [0b111,0b111,0b111,0b111,0b111].into();
            for (j, &val ) in dievals_vec.iter().enumerate() { 
                let idx = combo[j] as u8; 
                outcome.dievals.set(idx,val) ; 
                outcome.mask.set(idx,0);
            }
            outcome.arrangements = distinct_arrangements_for(dievals_vec);
            retval[i]=outcome;
            i+=1;
        }
    }
    retval
}

/// the set of all ways to roll different dice, as represented by a collection of index arrays
#[allow(clippy::eval_order_dependence)]
fn die_index_combos() ->[Vec<u8>;32]  { 
    let mut them:[Vec<u8>;32] = Default::default(); 
    let mut i=0; 
    for n in 1..=5 {
        for combo in (0..=4).combinations(n){ 
            them[i]= { let mut it=Vec::<u8>::new(); it.extend_from_slice(&combo); i+=1; it} 
        } 
    }
    them
}


/*-------------------------------------------------------------
UTILS
-------------------------------------------------------------*/

/// rudimentary factorial suitable for our purposes here.. handles up to fact(20) 
fn fact(n: u8) -> u64{
    if n<=1 {1} else { (n as u64)*fact(n-1) }
}

fn distinct_arrangements_for(dieval_vec:Vec<DieVal>)->u8{
    let counts = dieval_vec.iter().counts();
    let mut divisor:usize=1;
    let mut non_zero_dievals=0_u8;
    for count in counts { 
        if *count.0 != 0 { 
            divisor *= FACT[count.1] as usize ; 
            non_zero_dievals += count.1 as u8;
        }
    } 
    (FACT[non_zero_dievals as usize] as f64 / divisor as f64) as u8
}

/// count of arrangements that can be formed from r selections, chosen from n items, 
/// where order DOES or DOESNT matter, and WITH or WITHOUT replacement, as specified
fn n_take_r(n:usize, r:usize, order_matters:bool, with_replacement:bool)->u64{

    if order_matters { // order matters; we're counting "permutations" 
        if with_replacement {
            (n as u64).pow(r as u32)
        } else { // no replacement
            FACT[n] / FACT[n-r]  // this = FACT[n] when r=n
        }
    } else { // we're counting "combinations" where order doesn't matter; there are less of these 
        if with_replacement {
            FACT[n+r-1] / (FACT[r]*FACT[n-1])
        } else { // no replacement
            FACT[n] / (FACT[r]*FACT[n-r]) 
        }
    }

}

fn console_log(game:&GameState, app:&AppState, choice:Choice, ev:f32 ){
    app.progress_bar.println (
        format!("{:>}\t{}\t{:>4}\t{:>4.2}\t{}\t{}\t{:>30}", 
            game.rolls_remaining, game.yahtzee_is_wild, game.upper_bonus_deficit, ev, choice, game.sorted_dievals, game.sorted_open_slots 
        )
    );
}


/*-------------------------------------------------------------
SCORING FNs
-------------------------------------------------------------*/

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
    let mut iter=sorted_dievals.into_iter();
    let val = iter.next().unwrap();
    let val1=val; 
    let mut val1count =1; 
    let mut val2=0;
    let mut val2count =0;
    for val in iter {
        if val == 0 {return 0};
        if val1 == val {val1count+=1; continue;}
        if val2 == 0 {val2 = val; val2count=1; continue;}    
        if val2 == val {val2count+=1; continue;}
    }
    if val1==0 || val2==0 {return 0};
    if (val1count==3 && val2count==2) || (val2count==3 && val1count==2) {25} else {0}
}

fn score_chance(sorted_dievals:DieVals)->Score { sorted_dievals.into_iter().sum()  }

fn score_yahtzee(sorted_dievals:DieVals)->Score { 
    if sorted_dievals.get(0) == sorted_dievals.get(4) && sorted_dievals.get(0) != 0 {50} else {0}
}

/// reports the score for a set of dice in a given slot w/o regard for exogenous gamestate (bonuses, yahtzee wildcards etc)
fn score_slot(slot:Slot, sorted_dievals:DieVals)->Score{
    SCORE_FNS[slot as usize](sorted_dievals) 
}


/*-------------------------------------------------------------
Expected Value Core Functions 
-------------------------------------------------------------*/

/// returns the best slot and corresponding ev for final dice, given the slot possibilities and other relevant state 
fn best_slot_ev(game:GameState, app: &mut AppState) -> ChoiceEV  {

    let mut best_ev = 0.0; 
    let mut best_slot=STUB; 

    for mut slot_sequence in game.sorted_open_slots.permutations() { // We're trying out each ordering and choosing the best one

        // LEAF CALCS 
            // prep vars
                let mut tail_ev = 0.0;
                let head_slot = slot_sequence.pop();
                let mut _choice;
                let mut upper_deficit_now = game.upper_bonus_deficit ;
                let mut yahtzee_wild_now:bool = game.yahtzee_is_wild;

            // score slot itself w/o regard to game state 
                let mut head_ev = score_slot(head_slot, game.sorted_dievals); 

            // add upper bonus when needed total is reached
                if head_slot <= SIXES && upper_deficit_now>0 && head_ev>0 { 
                    if head_ev >= upper_deficit_now {head_ev += 35}; 
                    upper_deficit_now = upper_deficit_now.saturating_sub(head_ev) ;
                } 

            // special handling of "extra yahtzees" 
                let yahtzee_rolled = {score_yahtzee(game.sorted_dievals)==50}; 
                if yahtzee_rolled && game.yahtzee_is_wild { // extra yahtzee situation
                    if head_slot==SM_STRAIGHT {head_ev=30}; // extra yahtzees are valid in any lower slot, per wildcard rules
                    if head_slot==LG_STRAIGHT {head_ev=40}; 
                    if head_slot==FULL_HOUSE  {head_ev=25}; 
                    head_ev+=100; // extra yahtzee bonus per rules
                }
                if head_slot==YAHTZEE && yahtzee_rolled {yahtzee_wild_now = true} ;

        if slot_sequence.len > 0 { // proceed to include all the ev of remaining slots in this slot_sequence

            //prune unneeded state duplication when there's no chance of reaching upper bonus // TODO could maybe tune this a bit for speed
                let mut best_deficit = upper_deficit_now;
                for upper_slot in slot_sequence{ 
                    if upper_slot > 6 || best_deficit==0 {break};
                    best_deficit = best_deficit.saturating_sub(upper_slot*5);
                } // an impossible upper bonus is same ev as the "full deficit" scenario, where the ev may already be cached
                if best_deficit > 0 {upper_deficit_now=63}; 

            // we'll permutate and find max ev on the inside down below, but we'll use this sorted sequence as the key when we cache the max 
                slot_sequence.sort(); 

            // gather the collective ev for the remaining slots recursively
                ChoiceEV{choice:_choice, ev:tail_ev} = best_choice_ev( GameState{
                    yahtzee_is_wild: yahtzee_wild_now,
                    sorted_open_slots: slot_sequence, 
                    rolls_remaining: 3,
                    upper_bonus_deficit: upper_deficit_now,
                    sorted_dievals: game.sorted_dievals, 
                },app);
        }

        let ev = tail_ev + head_ev as f32 ; 
        if ev >= best_ev { best_ev = ev; best_slot = head_slot ; }

    } // end for slot_sequence...

    ChoiceEV{choice:best_slot, ev:best_ev}
}

/// returns the best selection of dice and corresponding ev, given slots left, existing dice, and other relevant state 
fn best_dice_ev(game:GameState, app: &mut AppState) -> ChoiceEV { 

    let mut best_selection:DieSet = 0b11111; // default selection is "all dice"
    let mut best_ev = 0.0; 
    if game.rolls_remaining==3 {// special case .. we always roll all dice on initial roll
        best_ev = avg_ev_for_selection(game,app,best_selection);
    } else { // iterate over all the possible ways to select dice and take the best outcome 
        for selection in 0b00000..=0b11111 { // each selection is a u8 encoded bitfield
            let avg_ev = avg_ev_for_selection(game,app,selection);
            if avg_ev > best_ev {
                best_ev = avg_ev; 
                best_selection = selection; 
            }
        }
    }
    ChoiceEV{choice:best_selection, ev:best_ev}
}

/// returns the average of all the expected values for rolling a selection of dice, given the game and app state
/// "selection" is the set of dice to roll, as represented their indexes in a 5-length array
#[inline(always)] // ~6% speedup 
fn avg_ev_for_selection(game:GameState, app: &mut AppState, selection:DieSet) -> f32 {
    let mut total_ev = 0.0;
    let mut newvals:DieVals; 

    let range = SELECTION_RANGES[selection as usize].clone(); // selection bitfield also acts as index into the cached ranges for corresponding outcomes 
    let mut outcomes_count:usize = 0; 
    for outcome in SELECTION_OUTCOMES[range].iter() { 
        //###### HOT CODE PATH #######
        newvals = Default::default(); 
        newvals.data = (game.sorted_dievals.data & outcome.mask.data) | outcome.dievals.data; // blit for the result after rolling selected dice. faster than looping
        newvals.sort();
        let ChoiceEV{choice: _choice, ev} = best_choice_ev( GameState{ 
            yahtzee_is_wild: game.yahtzee_is_wild, 
            sorted_open_slots: game.sorted_open_slots, 
            rolls_remaining: game.rolls_remaining-1,
            upper_bonus_deficit: game.upper_bonus_deficit,
            sorted_dievals: newvals, 
        }, app);
        outcomes_count += outcome.arrangements as usize; // we loop through die "combos" but we must sum all "perumtations"
        let added_ev = ev * outcome.arrangements as f32; // each combo's ev is weighted by its count of distinct arrangements
        total_ev += added_ev;
        //############################
        // eprintln!("{} {} {} {} {} {}", game.rolls_remaining, outcome.dievals, outcome.arrangements_count, newvals, ev, added_ev); 
    }
    total_ev/outcomes_count as f32 
}


/// returns the best game Choice along with its expected value, given relevant game state.
fn best_choice_ev(game:GameState,app: &mut AppState) -> ChoiceEV  { 

    if let Some(result) = app.ev_cache.get(&game) { return *result }; // return cached result if we have one 
    // cache contention here during constant cache writing effectively caps us to single threaded speeds
    // TODO consider periodically "freezing" chuncks of completed cache into read-only state for better multithreading


    let result = if game.rolls_remaining == 0 { 
        best_slot_ev(game,app)  // <-----------------
    } else { 
        best_dice_ev(game,app)  // <-----------------
    };

    // console_log(&game,app,result.choice,result.ev);

    // periodically update progress and save
    if game.rolls_remaining==0 { // TODO this will get slow. go back to a dedicated seen_slots hashset once multithreading is sorted out 
        let seen_slots = app.ev_cache.keys().any(|k| k.sorted_open_slots == game.sorted_open_slots); 
        if ! seen_slots  {
            app.save_periodically(600) ;
            console_log(&game,app, result.choice, result.ev);
            app.progress_bar.inc(FACT[game.sorted_open_slots.len as usize]);
        }
    }
    
    app.ev_cache.insert(game, result);
    result 
}


// fn score_slot_in_context(slot:Slot,dievals:DieVals,yahtzee_wild:bool,upper_deficit:u8) -> u8 {

//     // score slot itself w/o regard to game state 
//         let score = score_slot(slot, dievals); 

//     // add upper bonus when needed total is reached
//         if slot<=SIXES && upper_deficit>0 { 
//             upper_deficit = upper_deficit.saturating_sub(score) ;
//             if upper_deficit==0 {score += 35}; 
//         } 

//     // special handling of "extra yahtzees" 
//         let yahtzee_rolled = {score_yahtzee(dievals)==50}; 
//         if yahtzee_rolled && yahtzee_wild { // extra yahtzee situation
//             if slot==SM_STRAIGHT {score=30}; // extra yahtzees are valid in any lower slot, per wildcard rules
//             if slot==LG_STRAIGHT {score=40}; 
//             if slot==FULL_HOUSE  {score=25}; 
//             score+=100; // extra yahtzee bonus per rules
//         }
//         if slot==YAHTZEE && yahtzee_rolled {yahtzee_wild = true} ;

//     score
// }

// /// gather up expected values in a multithreaded bottom-up fashion
// fn build_cache(full_set:Slots) {

//     let cache:FxHashMap<GameState,ChoiceEV> = Default::default();

//     // first handle special case of the most leafy leaf calcs

//         // for each slot 
//         for slot in full_set { 

//             // for each yahtzee wild possibility
//             for &yahtzee_is_wild in [false, slot==YAHTZEE].iter().unique() {

//                 // for each upper bonus total 
//                 for upper_bonus_deficit in 0..63{

//                     // for each dievals outcome  
//                     for outcome in SELECTION_OUTCOMES[SELECTION_RANGES[0b11111].clone()].iter(){
//                         let game = GameState{
//                             sorted_dievals: outcome.dievals, //pre-cached dievals should already be sorted here
//                             rolls_remaining: 0,
//                             upper_bonus_deficit,
//                             yahtzee_is_wild,
//                             sorted_open_slots: Slots{data:slot as u64, len:1}, // set of a single slot
//                         };
//                         let score = score_slot_in_context(slot, outcome.dievals, yahtzee_is_wild, upper_bonus_deficit) as f32;
//                         cache.insert(game, ChoiceEV{ choice: slot, ev: score});
//                     } // end for each dievals outcome 

//                 } //end for each upper total

//             }//end for each yahtzee_is_wild

//         }//end for each slot 





//     // for each length
//     for slots_len in 1..=full_set.len{ 
//         let (tx, rx) = mpsc::channel();
//         let now = Instant::now();

//         // for each slotset (of above length)
//         for i in 0..=(full_set.len-slots_len) {
//             let slotset = full_set.subset(i,slots_len);
//             let yahtzee_may_be_wild = !slotset.into_iter().contains(&YAHTZEE); // yahtzees aren't wild whenever yahtzee slot is still available 
//             let chunk_size = fact(slotset.len) as usize / *CORES + 1 ; // one chunk per core (+1 chunk_size to "round up") 
//             let upper_bonus_deficits = slotset.missing_upper_slots().unique_upper_deficits(); 

//             // for each chunk of slot permutations 
//             for chunk in slotset.permutations().chunks(chunk_size).into_iter(){ 

//                 // "params" to thread
//                 let slotset_perms = chunk.collect_vec().into_iter(); // TODO some way to avoid collect_vec? https://stackoverflow.com/questions/42134874/are-there-equivalents-to-slicechunks-windows-for-iterators-to-loop-over-pairs
//                 let tx = tx.clone();
//                 let upper_bonus_deficits = upper_bonus_deficits.clone(); 

//                 thread::spawn(move ||{ 

//                     let mut score:Score= 0;
//                     let mut total:u32 = 0 ;
//                     let mut upper_deficit_now:f32 = 0.0 ;
//                     let mut best_choice:ChoiceEV = Default::default();

//                     // for each slot permutation in chunk
//                     for slot_perm in slotset_perms { 

//                         // for each yahtzee wild possibility
//                         for &yahtzee_is_wild in &[false,yahtzee_may_be_wild] {

//                             // for each upper bonus total 
//                             for upper_bonus_deficit in upper_bonus_deficits.clone(){

//                                 // for each rolls remaining
//                                 for rolls_remaining in 0..=3 {

//                                     // handle special case of the most leafy leaf calcs
//                                     if slots_len==1 && rolls_remaining==0 {
//                                         // for each dievals outcome 
//                                         for outcome in SELECTION_OUTCOMES[SELECTION_RANGES[0b11111].clone()].iter(){ 
//                                             let game = GameState{
//                                                 sorted_dievals: outcome.dievals, //pre-cached dievals should already be sorted here
//                                                 rolls_remaining,
//                                                 upper_bonus_deficit,
//                                                 yahtzee_is_wild,
//                                                 sorted_open_slots: slot_perm, //set of a single slot doesn't need sorting
//                                             };
//                                             let slot = slot_perm.get(0);
//                                             let score = score_slot_in_context(slot, outcome.dievals, yahtzee_is_wild, upper_bonus_deficit) as f32;
//                                             cache.insert(game, ChoiceEV{ choice: slot, ev: score});
//                                         } // end for each dievals outcome 
//                                     }

//                                     // choose best slot
//                                     else if rolls_remaining == 0 { 
                                        
//                                         // we'll permutate and find max ev on the inside down below, but we'll use this sorted sequence as the key when we cache the max 
//                                             slot_sequence.sort(); 

//                                         // gather the total ev for the all the slots in this order
//                                             ChoiceEV{choice:_choice, ev:tail_ev} = best_choice_ev( GameState{
//                                                 yahtzee_is_wild: yahtzee_wild_now,
//                                                 sorted_open_slots: slot_sequence, 
//                                                 rolls_remaining: 3,
//                                                 upper_bonus_deficit: upper_deficit_now,
//                                                 sorted_dievals: game.sorted_dievals, 
//                                             },app);

//                                         let ev = tail_ev + head_ev as f32 ; 
//                                         if ev >= best_ev { best_ev = ev; best_slot = head_slot ; }

//                                     }

//                                     // choose best dice to roll 
//                                     else if rolls_remaining == 1 || rolls_remaining ==2 {

//                                     }

//                                     // record ev at this state 
//                                     else if rolls_remaining == 3 {

//                                     }

                                   
//                                 } // end for each rolls

//                             } //end for each upper total
                            
//                         }//end for each yahtzee_is_wild


//                         // remember best slot_perm in chunk
//                         //  ... 

//                     }; // end for each permutation in chunk

//                     tx.send((chunk_best_perm, chunk_best_result)).unwrap();

//                 }); //end thread 

//             } // end for each chunk

//         } // end for each slot_set 

//     drop(tx); // would hang waiting for the cloned transmitter if it isn't explicity dropped 
//     let mut span_best_result:ChoiceEV = Default::default();
//     let mut span_best_perm:Slots= Default::default();
//     for rcvd in &rx { if rcvd.1.ev > span_best_result.ev {span_best_result = rcvd.1; span_best_perm = rcvd.0}; }
//     println!("{} {:?} {:.2?}",span_best_perm, span_best_result, now.elapsed()); 

//     } // end for each length

// }