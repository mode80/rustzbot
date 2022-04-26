#![allow(dead_code)] #![allow(unused_imports)] #![allow(unused_variables)]
#![allow(clippy::needless_range_loop)] #![allow(clippy::unusual_byte_groupings)] 

use std::{thread::{self, spawn, sleep}, sync::{Mutex, Arc, mpsc}, ops::Index, cmp::Ordering, time::Instant, default};
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

    build_cache(game,app);

    // let _ = best_choice_ev(game, app);
}

/*-------------------------------------------------------------
TYPE ALIASES
-------------------------------------------------------------*/
type Selection  = u8; // encodes a selection of which dice to roll where 0b11111 means "all five dice" and 0b00101 means "first and third"
type Choice     = u8; // represents EITHER the index of a chosen slot, OR a DieSet selection (below)
type DieVal     = u8; // a single die value 0 to 6 where 0 means "unselected"
type Slot       = u8; // a single slot with values ranging from ACES to CHANCE 
type Score      = u8;


/*-------------------------------------------------------------
SLOTS
-------------------------------------------------------------*/

#[derive(Debug,Clone,Copy,PartialEq,Serialize,Deserialize,Eq,PartialOrd,Ord,Hash,Default)]

struct Slots{
    // pub debug:[Slot;13],
    pub data:u64, // 13 Slot values of between 1 and 13 can be encoded within these 8 bytes, each taking 4 bits
    pub len:u8,
}

impl Slots {

    fn set(&mut self, index:u8, val:Slot) { 
        debug_assert!(index < self.len); 
        debug_assert!(index < 13); 
        let bitpos = 4*index; // widths of 4 bits per value 
        let mask = ! (0b1111_u64 << bitpos); // hole maker
        self.data = (self.data & mask) | ((val as u64) << bitpos ); // punch & fill hole
        // //for debugging only
        // self.debug[index as usize]=val;
    }

    fn get(&self, index:u8)->Slot{
        ((self.data >> (index*4)) & 0b1111_u64) as Slot 
    }


    /// swaps value at index a for index b using bit hack at https://graphics.stanford.edu/~seander/bithacks.html#SwappingValuesXOR
    fn swap(&mut self, a:u8, b:u8){ //no performance gain but looks cleaner in calling code
        let i = a*4; let j = b*4;// positions of bit sequences to swap 
        let x = ((self.data >> i) ^ (self.data >> j)) & 0b1111; // XOR temporary
        self.data ^= (x << i) | (x << j);
        // //for debugging only
        // let temp_a = self.debug[a];
        // self.debug[a]=self.debug[b];
        // self.debug[b]=temp_a;
     }

    fn push(&mut self, val:Slot){
        self.len +=1;
        self.set(self.len-1,val);
        // //for debugging only
        // self.debug[self.len as usize-1]=val;
     }

    fn truncate(&mut self, len:u8) {
        let mask = (2_u64).pow(len as u32 * 4)-1;
        self.data &= mask;
        self.len=len;
        // //for debugging only
        // self.debug.iter_mut().skip(len as usize).for_each(|x|*x=0); 
    }

    fn truncated(self, len:u8) -> Self {
        let mut self_copy = self;
        self_copy.truncate(len);
        self_copy
     }

    fn subset(self, start_idx: u8, max_len:u8) -> Self{
        debug_assert!(max_len<= self.len-start_idx);
        let mut self_copy = self;
        self_copy.data >>= start_idx*4;
        self_copy.truncate(max_len);
        // //for debugging only
        //     for i in 0..self_copy.len as usize {self_copy.debug[i] = self_copy.get(i as u8)};
        // //
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
    //     let mut ret:Self = default();
    //     let mut i=0;
    //     for s in ACES..CHANCE {
    //         if !self.into_iter().contains(&s) {ret.set(i,s); i+=1;}
    //     };
    //     ret
    // }

    fn missing_upper_slots(self) -> Self{
        let upper_slots= FxHashSet::<u8>::from_iter(self.into_iter().filter(|&x|x<=SIXES));
        let mut retval:Slots = default();
        for s in ACES..=SIXES { if !upper_slots.contains(&s) {retval.push(s)}; }
        retval
    }
 
    /// returns the unique and relevant "upper bonus total" shortfalls that could have occurred from the missing upper slots 
    fn upper_total_deficits(self) -> Vec<u8> { //impl Iterator<Item=u8> {  // TODO implement without allocating?
        let mut unique_totals:FxHashSet<u8> = default();
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
        let slot_idxs = self.missing_upper_slots().into_iter().filter(|&x|x<=SIXES).map(|x| x as usize).collect_vec();
        let score_idx_perms= repeat_n(0..=5, slot_idxs.len()).multi_cartesian_product();
        // for every permutation of entry indexes
        for score_idxs in score_idx_perms {
            // covert the list of entry indecis to a list of entry -scores-, then total them
            let tot = slot_idxs.iter().zip(score_idxs).map(|(i,ii)| UPPER_SCORES[*i][ii]).sum();
            // add the total to the set of unique totals 
            unique_totals.insert(tot);
        }

        //convert upper totals to upper deficits 
        let unique_deficits = unique_totals.into_iter().map(|x|63_u8.saturating_sub(x)).unique();

        // filter out the deficits that aren't relevant because they can't be covered by the upper slots remaining 
        // NOTE doing this filters out a lot of unneeded state space but means the lookup function must separately map extraneous deficits to 63 using relevant_deficit()
        let best_total = self.best_total_from_open_upper_slots();
        let mut retval = unique_deficits.filter(|x| *x == 63 || *x <= best_total).sorted().collect_vec(); //TODO 63 must be first but could remove sorted() somehow 
        retval.reverse();
        retval 

    }

    //converts the given deficit to 63 if the deficit can't be closed by the remaining upper slots 
    fn relevant_deficit(self,deficit:u8) -> u8{
        if deficit > self.best_total_from_open_upper_slots() {63} else {deficit}
    }

    fn best_total_from_open_upper_slots (self) -> u8{
        self.into_iter().fold(0,|a,x| if x<=SIXES {a + x*5} else {a}) 
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


// impl <const N:usize> From<[Slot; N]> for &mut Slots{
//     fn from(a: [Slot; N]) -> Self {
//         assert! (a.len() < 13);
//         let retval = &mut Slots{ len:a.len() as u8, data:default()};
//         for i in 0..N { retval.set(i as u8, a[i as usize]); }
//         retval 
//     }
// }

impl From<Vec<Slot>> for Slots{
    fn from(vec: Vec<Slot>) -> Self {
        assert! (vec.len() <= 13);
        let mut retval = Slots{ len:vec.len() as u8, data:default()};//, debug:default()};
        // //debug only
        // retval.debug[..vec.len()].copy_from_slice(&vec);
        // //
        for i in 0..vec.len() { retval.set(i as u8, vec[i as usize]); }
        retval 
    }
}
impl <const N:usize> From<[Slot; N]> for Slots{
    fn from(a: [Slot; N]) -> Self {
        assert! (a.len() <= 13);
        let mut retval = Slots{ len:a.len() as u8, data:default()};//, debug:default()};
        // // debug only
        // retval.debug[..N].copy_from_slice(&a);
        // // 
        for i in 0..N { retval.set(i as u8, a[i as usize]); }
        retval 
    }
}
impl <const N:usize>  From<&Slots> for [Slot; N]{ 
    fn from(slots: &Slots) -> Self {
        assert! ((slots.len as usize) <= N);
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
                self.slots.swap(self.i as u8, self.start);
                // let temp = self.slots.get(self.i as u8); // prep to swap
                // self.slots.set(self.i as u8, self.slots.get(self.start));
                // self.slots.set(self.start, temp);
            } else { // even iteration 
                self.slots.swap(self.c[self.i] as u8, self.i as u8);
                // let temp = self.slots.get(self.c[self.i] as u8); //prep to swap
                // self.slots.set(self.c[self.i] as u8, self.slots.get(self.i as u8));
                // self.slots.set(self.i as u8, temp);
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
    // debug:[u8;5],
    data:u16, // 5 dievals, each from 0 to 6, can be encoded in 2 bytes total, each taking 3 bits
}

impl DieVals {

    fn set(&mut self, index:u8, val:DieVal) { 
        let bitpos = 3*index; // widths of 3 bits per value
        let mask = ! (0b111_u16 << bitpos); // hole maker
        self.data = (self.data & mask) | ((val as u16) << bitpos ); // punch & fill hole
        // //for debug only
        // self.debug[index as usize]=val;
    }

    /// blit the 'from' dievals into the 'self' dievals with the help of a mask where 0 indicates incoming 'from' bits and 1 indicates none incoming 
    fn blit(&mut self, from:DieVals, mask:DieVals,){
        self.data = (self.data & mask.data) | from.data;
        // //for debugging only...
        // let debug:[DieVal;5] = self.into(); 
        // self.debug = debug;
    }

    /// merge the 'from' dievals into the 'self' using a bitwise OR 
    fn merge(&mut self, from:DieVals){
        self.data |= from.data;
        // //for debugging only...
        // let debug:[DieVal;5] = self.into(); 
        // self.debug = debug;
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
        DieVals{
            data: (a[4] as u16) << 12 | (a[3] as u16) <<9 | (a[2] as u16) <<6 | (a[1] as u16) <<3 | (a[0] as u16), 
            // debug:a
        }
    }
}

impl From<& DieVals> for [DieVal; 5]{ 
    fn from(dievals: &DieVals) -> Self {
        let mut temp:[DieVal;5] = default(); 
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
        Self { sorted_dievals: default(), rolls_remaining: 3, upper_bonus_deficit: 63, 
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
        // pb.set_style(ProgressStyle::default_bar()
        //     .template("{prefix} {wide_bar} {percent}% {pos:>4}/{len:4} {elapsed:>}/{duration} ETA:{eta}")
        //     .on_finish(ProgressFinish::Abandon)
        // );
        let init_capacity = slot_combos as usize * 252 * 64; // * 2 * 2; // roughly: slotcombos * diecombos * deficits * wilds * rolls
        let cachemap = if let Ok(bytes) = fs::read("ev_cache") { 
            ::bincode::deserialize(&bytes).unwrap() 
        } else {
            FxHashMap::with_capacity_and_hasher(init_capacity,default())
        };
        let cache_keys:Vec<&GameState> = cachemap.keys().into_iter().collect_vec();
        let former_ticks:u64 = cache_keys.into_iter().filter(|x|x.rolls_remaining ==0).map(|x|FACT[x.sorted_open_slots.len as usize] ).sum();
        // pb.inc(former_ticks);
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
static OUTCOMES:Lazy<[Outcome;1683]> = Lazy::new(all_selection_outcomes); 
static FACT:Lazy<[u64;21]> = Lazy::new(||{let mut a:[u64;21]=[0;21]; for i in 0..=20 {a[i]=fact(i as u8);} a});  // cached factorials
static CORES:Lazy<usize> = Lazy::new(num_cpus::get);


/*-------------------------------------------------------------
INITIALIZERS
-------------------------------------------------------------*/

/// this generates the ranges that correspond to the outcomes, within the set of all outcomes, indexed by a give selection 
fn selection_ranges() ->[Range<usize>;32]  { 
    let mut sel_ranges:[Range<usize>;32] = default();
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
    let mut retval:[Outcome;1683] = [default();1683];
    let mut outcome = Outcome::default();
    let mut i=0;
    for combo in die_index_combos(){
        outcome.dievals = default();
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
    let mut them:[Vec<u8>;32] = default(); 
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

/// my own deafult_free_fn
#[inline]
pub fn default<T: Default>() -> T {
   Default::default() 
}

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

    let mut best_selection:Selection = 0b11111; // default selection is "all dice"
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
fn avg_ev_for_selection(game:GameState, app: &mut AppState, selection:Selection) -> f32 {
    let mut total_ev = 0.0;
    let mut newvals:DieVals; 

    let range = SELECTION_RANGES[selection as usize].clone(); // selection bitfield also acts as index into the cached ranges for corresponding outcomes 
    let mut outcomes_count:usize = 0; 
    for outcome in OUTCOMES[range].iter() { 
        //###### HOT CODE PATH #######
        newvals= game.sorted_dievals;
        newvals.blit(outcome.dievals,outcome.mask);  
        // newvals = default(); 
        // newvals.data = (game.sorted_dievals.data & outcome.mask.data) | outcome.dievals.data; // blit for the result after rolling selected dice. faster than looping
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
        //  eprintln!("{} {} {} {} {} {}", game.rolls_remaining, outcome.dievals, outcome.arrangements_count, newvals, ev, added_ev); 
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


fn score_slot_in_context(slot:Slot,dievals:DieVals,yahtzee_wild:bool,upper_deficit:u8) -> u8 {

    /* score slot itself w/o regard to game state */
        let mut score = score_slot(slot, dievals); 

    /* add upper bonus when needed total is reached */
        if slot<=SIXES && upper_deficit>0 { 
            let upper_deficit = upper_deficit.saturating_sub(score) ;
            if upper_deficit==0 {score += 35}; 
        } 

    /* special handling of "extra yahtzees" */
        if yahtzee_wild && score_yahtzee(dievals)==50 { // extra yahtzee situation 
            if slot==SM_STRAIGHT {score=30}; // extra yahtzees are valid in any lower slot, per wildcard rules
            if slot==LG_STRAIGHT {score=40}; 
            if slot==FULL_HOUSE  {score=25}; 
            score+=100; // extra yahtzee bonus per rules
        }

    score
}

/// gather up expected values in a multithreaded bottom-up fashion
fn build_cache(game:GameState, app: &mut AppState) {
                    
    //TODO optimization: for slots where yahtzee_wild can't change and upperdeficits doesn't include 0, can permutating be skipped. thinking no... ie strategic use of chance
    let now = Instant::now();

    // first handle special case of the most leafy leaf calcs -- where there's one slot left and no rolls remaining
        for single_slot in game.sorted_open_slots {  // TODO: THREADS?
            let slots:Slots = [single_slot].into();//Slots{data:single_slot as u64, len:1}; //set of a single slot 
            for &yahtzee_is_wild in [false, single_slot!=YAHTZEE].iter().unique() {
                for upper_bonus_deficit in slots.upper_total_deficits(){
                    for outcome in OUTCOMES[SELECTION_RANGES[0b11111].clone()].iter(){
                        let game = GameState{
                            sorted_dievals: outcome.dievals, //pre-cached dievals should already be sorted here
                            sorted_open_slots: slots, 
                            rolls_remaining: 0, upper_bonus_deficit, yahtzee_is_wild,
                        };
                        let score = score_slot_in_context(single_slot, outcome.dievals, yahtzee_is_wild, upper_bonus_deficit) as f32;
                        let choice_ev = ChoiceEV{ choice: single_slot, ev: score};
                        // app.smart_cache_insert(&game, choice_ev);
                        app.ev_cache.insert(game, choice_ev);
                        println!("P {} {} {} {} {} {:.2?}", game.sorted_dievals, game.rolls_remaining, game.upper_bonus_deficit, game.yahtzee_is_wild, game.sorted_open_slots, choice_ev); 
         } } } }


    // for each length 
    for subset_len in 1..=game.sorted_open_slots.len{ 

        // for each slotset (of above length)
        // for i in 0..=(game.sorted_open_slots.len-slots_len) {
        for subset in game.sorted_open_slots.into_iter().combinations(subset_len as usize) {
            let mut subset:Slots = subset.into(); 
            subset.sort();
            let chunk_size = fact(subset.len) as usize / *CORES + 1 ; // one chunk per core (+1 chunk_size to "round up") 
            let yahtzee_may_be_wild = !subset.into_iter().contains(&YAHTZEE); // yahtzees aren't wild whenever yahtzee slot is still available 

            // for each upper bonus deficit 
            let upper_bonus_deficits = subset.upper_total_deficits(); 
            for upper_bonus_deficit in upper_bonus_deficits.clone() {

                // for each yahtzee wild possibility
                for yahtzee_is_wild in [false,yahtzee_may_be_wild].into_iter().unique() {

                    /* HANDLE SLOT SELECTION */
    
                    if subset_len>1 { //only select among > 1 slot
        
                        // let (tx, rx) = mpsc::channel();

                        let all_die_combos=&OUTCOMES[SELECTION_RANGES[0b11111].clone()];
                        for outcome in all_die_combos{

                            // for each chunk of slot permutations 
                            for chunk in subset.permutations().chunks(chunk_size).into_iter(){ 

                                // heap "arguments" to be passed into the thread
                                let slotset_perms = chunk.collect_vec().into_iter(); // TODO some way to avoid collect_vec? https://stackoverflow.com/questions/42134874/are-there-equivalents-to-slicechunks-windows-for-iterators-to-loop-over-pairs
                                // let tx = tx.clone();
                                // let cache = cache.clone(); // TODO needed?

                                // thread::spawn(move ||{ 

                                    let mut thread_best:ChoiceEV = default();

                                    // for each slot permutation in chunk
                                    for slot_perm in slotset_perms { 
                                                        
                                        let mut total = 0.0;
                                        let first_slot = slot_perm.get(0);
                                        let mut yahtzee_wild_now = yahtzee_is_wild;
                                        let mut upper_deficit_now = upper_bonus_deficit;
                                        thread_best = default();
                                        let head = slot_perm.subset(0, 1);
                                        let mut tail = if slot_perm.len > 1 {slot_perm.subset(1, slot_perm.len-1)} else {head};
                                        tail.sort();

                                        // find the collective ev for the all the slots when arranged like this 
                                        let mut sorted_dievals = outcome.dievals; 
                                        let mut rolls_remaining = 0;
                                        for slots_piece in [head,tail].into_iter().unique(){
                                            let game = GameState{
                                                sorted_dievals, 
                                                sorted_open_slots: slots_piece,
                                                rolls_remaining, 
                                                yahtzee_is_wild: yahtzee_wild_now, 
                                                upper_bonus_deficit: slots_piece.relevant_deficit(upper_deficit_now),
                                            };
                                            let choice_ev = app.ev_cache.get(&game).unwrap(); 
                                            total += choice_ev.ev;
                                            if slots_piece==head {
                                                if first_slot==YAHTZEE && choice_ev.ev>0.0 {yahtzee_wild_now=true;};
                                                if first_slot<=SIXES {
                                                    let deduct = (choice_ev.ev as u8) % 100; // the modulo 100 here removes any yathzee bonus from ev since that doesnt' count toward upper bonus total
                                                    upper_deficit_now = upper_deficit_now.saturating_sub(deduct);
                                                }; 
                                                rolls_remaining=3; // for upcoming tail lookup, we always want the ev for 3 rolls remaining
                                                sorted_dievals = DieVals::default() // for 3 rolls remaining, use "wildcard" representative dievals since dice don't matter when rolling all of them
                                            }
                                        } //end for slot_piece
                                        
                                        if total >= thread_best.ev { thread_best = ChoiceEV{ choice: first_slot, ev: total }};

                                    } // end for slot_perm 

                                    let gamestate = GameState {
                                        sorted_dievals: outcome.dievals, 
                                        sorted_open_slots: subset,
                                        rolls_remaining: 0, upper_bonus_deficit, yahtzee_is_wild,
                                    };
                            
                                    // tx.send((gamestate, thread_best)).unwrap(); //when is the right time to send?
                                    // NOTE goes under drop(tx) when changing to multithreaded
                                    // for (game, choice_ev) in &rx {  // receive transmissions from threads above with (GameState, ChoiceEV) tuples as candidates for best
                                    { let (game, choice_ev) = (gamestate, thread_best); 
                                        let cached = app.ev_cache.entry(game).or_default();
                                        if choice_ev.ev > cached.ev { 
                                            *cached=choice_ev;
                                            println!("S {} {} {} {} {} {:.2?}", game.sorted_dievals, game.rolls_remaining, game.upper_bonus_deficit, game.yahtzee_is_wild, game.sorted_open_slots, choice_ev); 
                                        } 
                                    } 

                                // });//end thread 
                            } // end for each chunk

                        } // end for outcome
                        
                        /* PROCESS THREAD OUTPUT */

                        // drop(tx); // would hang waiting for this template transmitter if not dropped 

                        // // save best from chunk
                        // for (game, choice_ev) in &rx {  // receive transmissions from threads above with (GameState, ChoiceEV) tuples as candidates for best
                        //     let cached = cache.entry(game).or_default();
                        //     if choice_ev.ev > cached.ev { *cached=choice_ev } 
                        //     println!("dievals: {} rr: {} ubd: {} yw: {} sos: {} {:?} {:.2?}",
                        //         game.sorted_dievals, game.rolls_remaining, game.upper_bonus_deficit, game.yahtzee_is_wild, game.sorted_open_slots, choice_ev, now.elapsed()); 
                        // } 

                    } // end if slot_len > 1

                    /* HANDLE DICE SELECTION */    //TODO Threads for this section?

                     // for each rolls remaining
                    for rolls_remaining in [1,2,3] { // TODO calculating and recording 200+ lookup outcomes on the 3rd roll is pointless  
                        let next_roll = rolls_remaining-1; //TODO other wildcard lookup opportunities like below? 
                        let die_combos = if rolls_remaining==3 {&OUTCOMES[0..1]} else {&OUTCOMES[ SELECTION_RANGES[0b11111].clone()]}; //OUTCOMES[0] has Dievals::default()
                        for starting_combo in die_combos {  // for every combo of all dice (except on first roll when we only need the default representative one)
                            let selections = if rolls_remaining ==3 { 0b11111..=0b11111 } else { 0b00000..=0b11111 }; //always select all dice on the initial roll
                            let mut best_selection_result = ChoiceEV::default();
                            for selection in selections.clone() { // try every selection against this starting_combo . TODO redundancies?
                                let mut total_evs_for_selection = 0.0; 
                                let mut outcomes_count:u64= 0; 
                                for selection_outcome in &OUTCOMES[ SELECTION_RANGES[selection].clone() ] {
                                    let mut newvals = starting_combo.dievals;
                                    newvals.blit(selection_outcome.dievals, selection_outcome.mask);
                                    newvals.sort(); // TODO lookup table? TODO faster to check equalities first?
                                    let gamestate_for_upcoming_roll = &GameState{
                                        sorted_dievals: newvals, 
                                        sorted_open_slots: subset, 
                                        upper_bonus_deficit: subset.relevant_deficit(upper_bonus_deficit), // TODO optimize
                                        yahtzee_is_wild,
                                        rolls_remaining: next_roll, // the trick is we average all the 'next roll' possibilities (which we calclated last)
                                    };
                                    let ev_for_this_selection_outcome = app.ev_cache.get(gamestate_for_upcoming_roll).unwrap().ev; 
                                    total_evs_for_selection += ev_for_this_selection_outcome * selection_outcome.arrangements as f32;// bake into upcoming aveage
                                    outcomes_count += selection_outcome.arrangements as u64; // we loop through die "combos" but we'll average all "perumtations"
                                }
                                let avg_ev_for_selection = total_evs_for_selection / outcomes_count as f32;
                                if avg_ev_for_selection > best_selection_result.ev{
                                    best_selection_result = ChoiceEV{choice:selection as u8, ev:avg_ev_for_selection};
                                }
                            }
                            let game = GameState{
                                sorted_dievals: starting_combo.dievals,  //presorted
                                sorted_open_slots: subset, 
                                upper_bonus_deficit, 
                                yahtzee_is_wild,
                                rolls_remaining, // this sitch
                            };
                            app.ev_cache.insert(game, best_selection_result);
                            println!("P {} {} {} {} {} {:.2?}", game.sorted_dievals, game.rolls_remaining, game.upper_bonus_deficit, game.yahtzee_is_wild, game.sorted_open_slots, best_selection_result); 
                        }

                    } // end for rolls_remaining

                } //end for each yahtzee_is_wild
            } //end for each upper deficit


        } // end for each slot_set 
    } // end for each length



} // end fn