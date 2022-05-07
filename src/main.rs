#![allow(dead_code)] #![allow(unused_imports)] #![allow(unused_variables)]
#![allow(clippy::needless_range_loop)] #![allow(clippy::unusual_byte_groupings)] 

use std::{cmp::{max, min}, fs::{self, File}, ops::Range, fmt::Display,};
use itertools::{Itertools, repeat_n};
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

    let game = GameState{sorted_open_slots: [1].into(), ..default()};//::default(); 
    let app = &mut AppState::new(&game);
    build_cache(game,app);
    for entry in &app.ev_cache {
        print_state_choice(entry.0, *entry.1)    
    }
     
    // let game = GameState::default();
    // let app = & mut AppState::new(&game);
    // build_cache(game,app);
    // app.save_cache();


}

/*-------------------------------------------------------------
TYPE ALIASES
-------------------------------------------------------------*/
type Choice     = u8; // represents EITHER the index of a chosen slot, OR a DieSet selection (below)
type DieVal     = u8; // a single die value 0 to 6 where 0 means "unselected"
type Slot       = u8; // a single slot with values ranging from ACES to CHANCE 
type Score      = u8;


/*-------------------------------------------------------------
SLOTS
-------------------------------------------------------------*/

#[derive(Debug,Clone,Copy,PartialEq,Serialize,Deserialize,Eq,PartialOrd,Ord,Hash,Default)]

struct Slots{
    pub data:u64, // 13 Slot values of between 1 and 13 can be encoded within these 8 bytes, each taking 4 bits
    pub len:u8,
}

/*the following LLDB command will format Slots with meaningful values in the debugger 
    type summary add --summary-string "Slots ${var.data[0-3]%u} ${var.data[4-7]%u} ${var.data[8-11]%u} ${var.data[12-15]%u} ${var.data[16-19]%u} ${var.data[20-23]%u} ${var.data[24-27]%u} ${var.data[28-31]%u} ${var.data[32-35]%u} ${var.data[36-39]%u} ${var.data[40-43]%u} ${var.data[44-47]%u} ${var.data[48-51]%u}" "yahtzeebot::Slots"
*/

impl Slots {

    // fn encode_sorted_to_u16(self) -> u16 {
    //     let mut ret:u16 = 0;
    //     self.to().for_each(|x| ret |= 1<<x); 
    //     ret
    // }

    // fn decode_u16(input:u16) -> Self {
    //     let mut mut_input = input;
    //     let mut trailing_zeros=0;
    //     let mut slots = Slots::default();
    //     while trailing_zeros<13 {
    //         trailing_zeros=mut_input.trailing_zeros() as u8;
    //         slots.push(trailing_zeros);
    //         mut_input ^= 1<<trailing_zeros;
    //     }
    //     slots
    // }

    fn set(&mut self, index:u8, val:Slot) { 
        debug_assert!(index < self.len); 
        debug_assert!(index < 13); 
        let bitpos = 4*index; // widths of 4 bits per value 
        let mask = ! (0b1111_u64 << bitpos); // hole maker
        self.data = (self.data & mask) | ((val as u64) << bitpos ); // punch & fill hole
    }

    fn get(&self, index:u8)->Slot{
        ((self.data >> (index*4)) & 0b1111_u64) as Slot 
    }

    fn push(&mut self, val:Slot){
        self.len +=1;
        self.set(self.len-1,val);
    }

    fn removed(self, val:Slot)->Self{
        match self.to().position(|x|x==val) {
            Some(idx) =>  {
                let front:Slots = self.truncated(idx as u8);
                let back = self.data & !(2_u64.pow((idx as u32+1)*4)-1);
                Slots{data:front.data | (back>>4), len: self.len-1}
            },
            None => {self}
        }
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

    // // given this set of slots, what set of slots have previously been played? (ie the inverse set)
    // fn previously_played (self) -> Self{
    //     let mut ret:Self = default();
    //     let mut i=0;
    //     for s in ACES..CHANCE {
    //         if !self.ii().contains(&s) {ret.set(i,s); i+=1;}
    //     };
    //     ret
    // }

    fn used_upper_slots(self) -> Self{
        let upper_slots= FxHashSet::<u8>::from_iter(self.to().filter(|&x|x<=SIXES));
        let mut retval:Slots = default();
        for s in ACES..=SIXES { if !upper_slots.contains(&s) {retval.push(s)}; }
        retval
    }
 
    /// returns the unique and relevant "upper bonus total" that could have occurred from the previously used upper slots 
    fn relevant_upper_totals(self) -> impl Iterator<Item=u8>   {  // TODO implement without allocating? w impl Iterator<Item=u8>  
        let mut totals:FxHashSet<u8> = default();
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
        let used_slot_idxs = &self.used_upper_slots().to().filter(|&x|x<=SIXES).map(|x| x as usize).collect_vec(); 
        let used_score_idx_perms= repeat_n(0..=5, used_slot_idxs.len()).multi_cartesian_product();
        // for every permutation of entry indexes
        for used_score_idxs in used_score_idx_perms {
            // covert the list of entry indecis to a list of entry -scores-, then total them
            let tot = used_slot_idxs.iter().zip(used_score_idxs).map(|(i,ii)| UPPER_SCORES[*i][ii]).sum();
            // add the total to the set of unique totals 
            totals.insert(min(tot,63));
        }
        totals.insert(0); // 0 is always relevant and must be added here explicitly when there are no used upper slots 

        // filter out the totals that aren't relevant because they can't be reached by the upper slots remaining 
        // NOTE this filters out a lot of unneeded state space but means the lookup function must map extraneous deficits to a default using relevant_total()
        let best_current_slot_total = self.best_total_from_current_upper_slots();
        totals.to().filter/*keep!*/(move |used_slots_total| 
            *used_slots_total==0 || // always relevant 
            *used_slots_total + best_current_slot_total >= 63 // totals must reach the bonus threshhold to be relevant
        )

    }

    //converts the given total to a default if the bonus threshold can't be reached 
    fn relevant_total(self,given_total:u8) -> u8{
        if self.best_total_from_current_upper_slots() + given_total >= 63 {given_total} else {0}
    }

    fn best_total_from_current_upper_slots (self) -> u8{
        let mut sum=0;
        for x in self { if x>6 {break} else {sum+=x;} }
        sum*5
    }

}

impl Display for Slots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // let a:[Slot;13] = self.into(); 
        // write!(f,"{:?}",temp.ii().filter(|x|*x!=0).collect_vec()) 
        self.to().for_each(|x| write!(f,"{}_",x).unwrap());
        Ok(())
        //     "{: <2} {: <2} {: <2} {: <2} {: <2} {: <2} {: <2} {: <2} {: <2} {: <2} {: <2} {: <2} {: <2}",
        //     a[0],a[1],a[2],a[3],a[4],a[5],a[6],a[7],a[8],a[9],a[10],a[11],a[12]
        // )//.it().map(|x| format!("{: <2} ",x)).collect::<String>()) 
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
        let mut retval = Slots{ len:vec.len() as u8, data:default()};
        for i in 0..vec.len() { retval.set(i as u8, vec[i as usize]); }
        retval 
    }
}
impl From<&[Slot]> for Slots{
    fn from(a: &[Slot]) -> Self {
        assert! (a.len() <= 13);
        let mut retval = Slots{ len:a.len() as u8, data:default()};
        for i in 0..a.len() { retval.set(i as u8, a[i as usize]); }
        retval 
    }
}
impl <const N:usize> From<[Slot; N]> for Slots{
    fn from(a: [Slot; N]) -> Self {
        assert! (a.len() <= 13);
        let mut retval = Slots{ len:a.len() as u8, data:default()};
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
DieVals
-------------------------------------------------------------*/

#[derive(Debug,Clone,Copy,PartialEq,Serialize,Deserialize,Eq,PartialOrd,Ord,Hash,Default)]

struct DieVals{
    data:u16, // 5 dievals, each from 0 to 6, can be encoded in 2 bytes total, each taking 3 bits
}

/* the following LLDB command will format Slots with meaningful values in the debugger 
    type summary add --summary-string "${var.data[0-2]%u} ${var.data[3-5]%u} ${var.data[6-8]%u} ${var.data[9-11]%u} ${var.data[12-14]%u}" "yahtzeebot::DieVals"
*/

impl DieVals {

    fn set(&mut self, index:u8, val:DieVal) { 
        let bitpos = 3*index; // widths of 3 bits per value
        let mask = ! (0b111_u16 << bitpos); // hole maker
        self.data = (self.data & mask) | ((val as u16) << bitpos ); // punch & fill hole
    }

    /// blit the 'from' dievals into the 'self' dievals with the help of a mask where 0 indicates incoming 'from' bits and 1 indicates none incoming 
    fn blit(&mut self, from:DieVals, mask:DieVals,){
        self.data = (self.data & mask.data) | from.data;
    }

    fn get(&self, index:u8)->DieVal{
        ((self.data >> (index*3)) & 0b111) as DieVal
    }

}

impl Display for DieVals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,"{}{}{}{}{}",self.get(4), self.get(3),self.get(2),self.get(1),self.get(0)) 
    }
}

impl From<Vec<DieVal>> for DieVals{
    fn from(v: Vec<DieVal>) -> Self {
        let mut a:[DieVal;5]=default();
        a.copy_from_slice(&v[0..5]);
        a.into()
    }
}

impl From<[DieVal; 5]> for DieVals{
    fn from(a: [DieVal; 5]) -> Self {
        DieVals{
            data: (a[4] as u16) << 12 | (a[3] as u16) <<9 | (a[2] as u16) <<6 | (a[1] as u16) <<3 | (a[0] as u16), 
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

// /* YahtzeeStatus*/
// #[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Clone, Copy, Serialize, Deserialize)]
// enum YahtzeeStatus {
//     ZEROED,
//     ROLLED,
//     OPEN,
// }

/*-------------------------------------------------------------
GameState
-------------------------------------------------------------*/
#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Clone, Copy, Serialize, Deserialize)]
struct GameState{
    rolls_remaining:u8, // 3 bits 
    sorted_dievals:DieVals, //15 bits 
    sorted_open_slots:Slots, // 52 bits... or 1+2+2+3+3+3+3+4+4+4+4+4+4=41 .. or 13 sorted 
    upper_total:u8, // 6 bits 
    yahtzee_bonus_avail:bool, // 1 bit // TODO replace with below
}
// impl std::hash::Hash for GameState{
//     fn hash<H: Hasher>(&self, hasher: &mut H) {
//         todo!();
//     }
// }    
// impl PartialEq for GameState{
//     fn eq(&self, other: &Self) -> bool {
//         self.sorted_dievals == other.sorted_dievals 
//         && self.rolls_remaining == other.rolls_remaining 
//         && self.upper_total == other.upper_total
//         && self.yahtzee_is_wild == other.yahtzee_is_wild 
//         && self.sorted_open_slots == other.sorted_open_slots
//     }
// }    

impl Default for GameState{
    fn default() -> Self {
        Self { sorted_dievals: default(), rolls_remaining: 3, upper_total: 63, 
            yahtzee_bonus_avail: false, sorted_open_slots: [1,2,3,4,5,6,7,8,9,10,11,12,13].into(),
        }
    }
 }
 impl GameState{ 
    /// calculate relevant counts for gamestate: required lookups and saves
    fn counts(self) -> GameStateCounts {

        let mut lookups:u64 = 0;
        let mut saves:usize =0;
        for subset_len in 1..=self.sorted_open_slots.len{ 
            for slots_vec in self.sorted_open_slots.to().combinations(subset_len as usize) {
                let slots:Slots =slots_vec.into(); 
                let joker_rules = !slots.to().contains(&YAHTZEE); // yahtzees aren't wild whenever yahtzee slot is still available 
                for upper_bonus_deficit in slots.relevant_upper_totals() {
                    for yahtzee_bonus_avail in [false,joker_rules].to().unique() {
                        let slot_lookups = (subset_len as u64 * if subset_len==1{1}else{2} as u64) * 252 ;// * subset_len as u64;
                        let dice_lookups = 848484; // verified by counting up by 1s in the actual loop
                        lookups += dice_lookups + slot_lookups;
                        saves+=1;
                        // println!("+({}+{})={} | {} ", dice_lookups, slot_perms, lookups, saves, );
        }}}}
        
        GameStateCounts{ lookups, saves } 
    }
}
#[derive(Debug)]
struct GameStateCounts {
    lookups:u64,
    saves:usize 
}


/*-------------------------------------------------------------
AppState
-------------------------------------------------------------*/
struct AppState{
    bar:ProgressBar,
    ev_cache:FxHashMap<GameState,ChoiceEV>,
}
impl AppState{
    fn new(game: &GameState) -> Self{
        let GameStateCounts{ lookups, saves} = game.counts();

        let bar = ProgressBar::new(lookups);
        bar.set_draw_rate(1);
        bar.set_style(ProgressStyle::default_bar()
            .template("{prefix} {wide_bar} {percent}% {pos:>4}/{len:4} {elapsed:>}/{duration} ETA:{eta}")
            .on_finish(ProgressFinish::AtCurrentPos)
        );

        let init_hash_capacity = saves; 
        let ev_cache = if let Ok(bytes) = fs::read("ev_cache") { 
            ::bincode::deserialize(&bytes).unwrap() 
        } else {
            FxHashMap::with_capacity_and_hasher(init_hash_capacity, default())
        };

        Self{bar, ev_cache}
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
 
const SCORE_FNS:[fn(sorted_dievals:DieVals)->Score;14] = [
    score_aces, // duplicate placeholder so indices align more intuitively with categories 
    score_aces, score_twos, score_threes, score_fours, score_fives, score_sixes, 
    score_3ofakind, score_4ofakind, score_fullhouse, score_sm_str8, score_lg_str8, score_yahtzee, score_chance, 
];

static SELECTION_RANGES:Lazy<[Range<usize>;32]> = Lazy::new(selection_ranges); 
static OUTCOMES:Lazy<[Outcome;1683]> = Lazy::new(all_selection_outcomes); 
static FACT:Lazy<[u64;21]> = Lazy::new(||{let mut a:[u64;21]=[0;21]; for i in 0..=20 {a[i]=fact(i as u8);} a});  // cached factorials
static SORTED_DIEVALS:Lazy<FxHashMap<DieVals,DieVals>> = Lazy::new(sorted_dievals); 
// static CORES:Lazy<usize> = Lazy::new(num_cpus::get);

/*-------------------------------------------------------------
INITIALIZERS
-------------------------------------------------------------*/

fn sorted_dievals() -> FxHashMap<DieVals, DieVals> {
    let mut map = FxHashMap::default();
    repeat_n( 0_u8..=6 , 5).multi_cartesian_product().for_each(|x| {
        let mut sorted = x.clone();
        sorted.sort_unstable();
        map.insert(x.into(), sorted.into() );
    });
    map
}

/// this generates the ranges that correspond to the outcomes, within the set of all outcomes, indexed by a give selection 
fn selection_ranges() ->[Range<usize>;32]  { 
    let mut sel_ranges:[Range<usize>;32] = default();
    let mut s = 0;
    sel_ranges[0] = 0..1;
    for (i,combo) in die_index_combos().to().enumerate(){
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
        for dievals_vec in [1,2,3,4,5,6_u8].to().combinations_with_replacement(combo.len()){ 
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
/// allow use of it() where into_iter() is normally required. so wrong but so right. 
    trait ItShortcut {
        type Item;
        type IntoIter: Iterator<Item = Self::Item>;
        fn to(self) -> Self::IntoIter; 
    }
    impl<T: IntoIterator> ItShortcut for T{ 
        type Item=T::Item;
        type IntoIter= T::IntoIter;//Iterator<Item = T::Item>;
        fn to(self) -> Self::IntoIter { self.into_iter() }
    }

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

fn log_state_choice(state: &GameState, choice_ev:ChoiceEV, app:&AppState){
    // if state.rolls_remaining==0 {
    //     app.bar.println(format!("S\t{: >6.2?}\t{:_^5}\t{:2?}\t{}\t{:2?}\t{}\t{: <29}",
    //         choice_ev.ev, choice_ev.choice, state.rolls_remaining, state.sorted_dievals, state.upper_total, 
    //         if state.yahtzee_bonus_avail {"Y"}else{""}, state.sorted_open_slots.to_string())); 
    // } else {
    //     app.bar.println(format!("D\t{: >6.2?}\t{:05b}\t{:2?}\t{}\t{:2?}\t{}\t{: <29}",
    //         choice_ev.ev, choice_ev.choice, state.rolls_remaining, state.sorted_dievals, state.upper_total, 
    //         if state.yahtzee_bonus_avail {"Y"}else{""}, state.sorted_open_slots.to_string())); 
    // };
}


fn print_state_choice(state: &GameState, choice_ev:ChoiceEV){
    if state.rolls_remaining==0 {
        println!("S\t{: >6.2?}\t{:_^5}\t{:2?}\t{}\t{:2?}\t{}\t{: <29}",
            choice_ev.ev, choice_ev.choice, state.rolls_remaining, state.sorted_dievals, state.upper_total, 
            if state.yahtzee_bonus_avail {"Y"}else{""}, state.sorted_open_slots.to_string()); 
    } else {
        println!("D\t{: >6.2?}\t{:05b}\t{:2?}\t{}\t{:2?}\t{}\t{: <29}",
            choice_ev.ev, choice_ev.choice, state.rolls_remaining, state.sorted_dievals, state.upper_total, 
            if state.yahtzee_bonus_avail {"Y"}else{""}, state.sorted_open_slots.to_string()); 
    };
}


/*-------------------------------------------------------------
SCORING FNs
-------------------------------------------------------------*/

fn score_upperbox(boxnum:Slot, sorted_dievals:DieVals)->Score{
   sorted_dievals.to().filter(|x| *x==boxnum).sum()
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
fn score_lg_str8(sorted_dievals:    DieVals)->Score{ 
    if sorted_dievals==[1,2,3,4,5].into() || sorted_dievals==[2,3,4,5,6].into() {40} else {0}
}

// The official rule is that a Full House is "three of one number and two of another"
fn score_fullhouse(sorted_dievals:DieVals) -> Score { 
    let mut iter=sorted_dievals.to();
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

fn score_chance(sorted_dievals:DieVals)->Score { sorted_dievals.to().sum()  }

fn score_yahtzee(sorted_dievals:DieVals)->Score { 
    if sorted_dievals.get(0) == sorted_dievals.get(4) && sorted_dievals.get(0) != 0 {50} else {0}
}

/// reports the score for a set of dice in a given slot w/o regard for exogenous gamestate (bonuses, yahtzee wildcards etc)
fn score_slot(slot:Slot, sorted_dievals:DieVals)->Score{
    SCORE_FNS[slot as usize](sorted_dievals) 
}

fn score_slot_in_context(game:&GameState) -> u8 {

    /* score slot itself w/o regard to game state */
        let slot = game.sorted_open_slots.get(0);
        let mut score = score_slot(slot, game.sorted_dievals); 

    /* add upper bonus when needed total is reached */
        if slot<=SIXES && game.upper_total>0 { 
            let new_deficit = game.upper_total.saturating_sub(score) ;
            if new_deficit==0 {score += 35}; 
        } 

    /* special handling of "joker rules" */
            //"Score the total of all 5 dice in the appropriate Upper Section box. 
            // If this box has already been filled in, score as follows in any open Lower Section box  .
            // If the appropriate Upper Section box and all Lower Section boxes are filled in,
            // you must enter a zero in any open Upper Section box."
        let just_rolled_yahtzee = score_yahtzee(game.sorted_dievals)==50;
        let joker_rules_in_play = slot!=YAHTZEE; // joker rules in effect when the yahtzee slot is not open 
        if just_rolled_yahtzee && joker_rules_in_play{ // standard scoring applies against the yahtzee dice except ... 
            if slot==FULL_HOUSE  {score=25}; 
            if slot==SM_STRAIGHT {score=30}; 
            if slot==LG_STRAIGHT {score=40}; 
        }

    /* special handling of "extra yahtzee" bonus per rules*/
        if just_rolled_yahtzee && game.yahtzee_bonus_avail { 
           score+=100; // extra yahtzee bonus per rules
        }

    score
}

fn best_choice_ev(game:GameState, app: &mut AppState) -> ChoiceEV{
    debug_assert!(app.ev_cache.is_empty());
    build_cache(game, app);
    *app.ev_cache.get(&game).unwrap()
}

/*-------------------------------------------------------------
BUILD CACHE
-------------------------------------------------------------*/

/// gather up expected values in a multithreaded bottom-up fashion
fn build_cache(game:GameState, app: &mut AppState) {
    let sorted = SORTED_DIEVALS.clone();
    let all_die_combos=&OUTCOMES[SELECTION_RANGES[0b11111].clone()];
    let placeholder_dievals= &OUTCOMES[0..=0]; //OUTCOMES[0] == [Dievals::default()]
    let mut leaf_cache = FxHashMap::<GameState,ChoiceEV>::default();

    // first handle special case of the most leafy leaf calcs -- where there's one slot left and no rolls remaining
    for single_slot in game.sorted_open_slots {  // TODO: THREADS?
        let slot:Slots = [single_slot].into();//Slots{data:single_slot as u64, len:1}; //set of a single slot 
        let joker_rules_in_play = single_slot!=YAHTZEE; // joker rules in effect when the yahtzee slot is not open 
        for yahtzee_bonus_available in [false, joker_rules_in_play].to().unique() { // yahtzee bonus -might- be available when joker rules are in play 
            for upper_total in slot.relevant_upper_totals(){
                for outcome in all_die_combos{
                    let state = GameState{
                        rolls_remaining: 0, 
                        sorted_dievals: outcome.dievals, //pre-cached dievals should already be sorted here
                        sorted_open_slots: slot, 
                        upper_total, 
                        yahtzee_bonus_avail: yahtzee_bonus_available,
                    };
                    let score = score_slot_in_context(&state) as f32;
                    let choice_ev = ChoiceEV{ choice: single_slot, ev: score};
                    leaf_cache.insert(state, choice_ev);
                    // log_state_choice(&state, choice_ev, app)
    } } } }

    // for each length 
    for slots_len in 1..=game.sorted_open_slots.len{ 

        // for each slotset (of above length)
        for slots_vec in game.sorted_open_slots.to().combinations(slots_len as usize) {
            let mut slots:Slots = slots_vec.into(); 
            slots.sort(); //TODO don't these come out of combinations already sorted? avoidable?
            let joker_rules_in_play = !slots.to().contains(&YAHTZEE); // joker rules are in effect whenever the yahtzee slot is already filled 

            // for each upper total 
            for upper_total in slots.relevant_upper_totals() {

                // for each yathzee bonu possibility 
                for yahtzee_bonus_available in [false,joker_rules_in_play].to().unique() {// bonus always unavailable unless yahtzees are wild first

                    app.bar.inc(848484); // advance the progress bar by the number of cache reads coming up for dice selection 
                    app.bar.inc(252 * slots_len as u64 *if slots_len ==1{1}else{2}); // advance for slot selection cache reads

                    // for each rolls remaining
                    for rolls_remaining in [0,1,2,3] { 

                        let die_combos = if rolls_remaining==3 {placeholder_dievals} else {all_die_combos}; 

                        let built_from_threads = die_combos.into_par_iter().fold(FxHashMap::<GameState,ChoiceEV>::default, |mut built_this_thread, die_combo|{  

                            if rolls_remaining==0  { 
                            /* HANDLE SLOT SELECTION */

                                let mut slot_choice_ev:ChoiceEV = default();

                                for slot in slots {

                                    //joker rules say extra yahtzees must be played in their matching upper slot if it's available
                                    let first_dieval =die_combo.dievals.get(0);
                                    let joker_rules_matter = joker_rules_in_play && score_yahtzee(die_combo.dievals)>0 && slots.to().contains(&first_dieval);
                                    let head:Slots = if joker_rules_matter { // then the head slot choice must be the upper slot matching the dice (all being the same)
                                        [first_dieval].into() // slot matches the dievals
                                    } else { // outside a joker-rules forced-choice situation, we'll try each starting slot in turn 
                                        [slot].into() 
                                    };

                                    let mut yahtzee_bonus_avail_now = yahtzee_bonus_available;
                                    let mut upper_total_now = upper_total;
                                    let mut dievals_or_wildcard = die_combo.dievals; 
                                    let tail = if slots_len > 1 { slots.removed(head.get(0)) } else {head};
                                    let mut head_plus_tail_ev = 0.0;
    
                                    // find the collective ev for the all the slots with this iteration's slot being first 
                                    // do this by summing the ev for the first (head) slot with the ev value that we look up for the remaining (tail) slots
                                    let mut rolls_remaining = 0;
                                    for slots_piece in [head,tail].to().unique(){
                                        let state = &GameState{
                                            rolls_remaining, 
                                            sorted_dievals: dievals_or_wildcard,
                                            sorted_open_slots: slots_piece, 
                                            upper_total: slots_piece.relevant_total(upper_total_now), 
                                            yahtzee_bonus_avail: yahtzee_bonus_avail_now,
                                        };
                                        let cache = if slots_piece==head { &leaf_cache } else { &app.ev_cache};
                                        let choice_ev = cache.get(state).unwrap(); 
                                        if slots_piece==head { // on the first pass only.. 
                                            //going into tail slots next, we may need to adjust the state based on the head choice
                                            if choice_ev.choice <=SIXES { // adjust upper total for the next pass 
                                                let added = (choice_ev.ev as u8) % 100; // the modulo 100 here removes any yathzee bonus from ev since that doesnt' count toward upper bonus total
                                                upper_total_now = min(63, upper_total_now + added);
                                            } else if choice_ev.choice==YAHTZEE { // adjust yahtzee related state for the next pass
                                                if choice_ev.ev>0.0 {yahtzee_bonus_avail_now=true;};
                                            }
                                            rolls_remaining=3; // for upcoming tail lookup, we always want the ev for 3 rolls remaining
                                            dievals_or_wildcard = DieVals::default() // for 3 rolls remaining, use "wildcard" representative dievals since dice don't matter when rolling all of them
                                        }
                                        head_plus_tail_ev += choice_ev.ev;
                                    } //end for slot_piece
                                    if head_plus_tail_ev >= slot_choice_ev.ev { slot_choice_ev = ChoiceEV{ choice: slot, ev: head_plus_tail_ev}};
                                    
                                    if joker_rules_matter {break};// if joker-rules-matter we were forced to choose one slot, so we can skip trying the rest  
                                }
                                
                                let state = GameState {
                                    sorted_dievals: die_combo.dievals, 
                                    sorted_open_slots: slots,
                                    rolls_remaining: 0, 
                                    upper_total, 
                                    yahtzee_bonus_avail: yahtzee_bonus_available ,
                                };
                                built_this_thread.insert( state, slot_choice_ev);
                                // log_state_choice(&state, slot_choice_ev, app)

                            } else { //if rolls_remaining > 0  
                            /* HANDLE DICE SELECTION */    

                                let next_roll = rolls_remaining-1; //TODO other wildcard lookup opportunities like below? 
                                let selections = if rolls_remaining ==3 { 0b11111..=0b11111 } else { 0b00000..=0b11111 }; //always select all dice on the initial roll
                                let mut best_dice_choice_ev = ChoiceEV::default();
                                for selection in selections { // try every selection against this starting_combo . TODO redundancies?
                                    let mut total_ev_for_selection = 0.0; 
                                    let mut outcomes_count:u64= 0; 
                                    for selection_outcome in &OUTCOMES[ SELECTION_RANGES[selection].clone() ] {
                                        let mut newvals = die_combo.dievals;
                                        newvals.blit(selection_outcome.dievals, selection_outcome.mask);
                                        newvals = sorted[&newvals]; 
                                        let state = GameState{
                                            sorted_dievals: newvals, 
                                            sorted_open_slots: slots, 
                                            upper_total: slots.relevant_total(upper_total), // TODO optimize ?
                                            yahtzee_bonus_avail: yahtzee_bonus_available, 
                                            rolls_remaining: next_roll, // we'll average all the 'next roll' possibilities (which we'd calclated last) to get ev for 'this roll' 
                                        };
                                        let ev_for_this_selection_outcome = app.ev_cache.get(&state).unwrap().ev; 
                                        total_ev_for_selection += ev_for_this_selection_outcome * selection_outcome.arrangements as f32;// bake into upcoming aveage
                                        outcomes_count += selection_outcome.arrangements as u64; // we loop through die "combos" but we'll average all "perumtations"
                                    }
                                    let avg_ev_for_selection = total_ev_for_selection / outcomes_count as f32;
                                    let actual_selection = [0,1,2,4,8,16,3,5,6,9,10,12,17,18,20,24,7,11,13,14,19,21,22,25,26,28,15,23,27,29,30,31][selection]; // TODO temp hack 
                                    if avg_ev_for_selection > best_dice_choice_ev.ev{
                                        best_dice_choice_ev = ChoiceEV{choice:actual_selection as u8, ev:avg_ev_for_selection};
                                    }
                                }
                                let state = GameState{
                                        sorted_dievals: die_combo.dievals,  
                                        sorted_open_slots: slots, 
                                        upper_total, 
                                        yahtzee_bonus_avail: yahtzee_bonus_available, 
                                        rolls_remaining, 
                                    }; 
                                built_this_thread.insert(state,best_dice_choice_ev);
                                // log_state_choice(&state, best_dice_choice_ev, app)

                            } // endif roll_remaining...  

                            built_this_thread

                        }).reduce(FxHashMap::<GameState,ChoiceEV>::default, |mut a,built_from_thread|{
                            a.extend(&built_from_thread); a 
                        }); // end die_combos.par_into_iter() 

                        app.ev_cache.extend(&built_from_threads);

                    } // end for rolls_remaining
                } //end for each yahtzee_is_wild
            } //end for each upper total 
        } // end for each slot_set 
    } // end for each length
} // end fn