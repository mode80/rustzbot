#![allow(dead_code)] #![allow(unused_imports)] #![allow(unused_variables)]
#![allow(clippy::needless_range_loop)] #![allow(clippy::unusual_byte_groupings)] 
use std::{cmp::{max, min}, fs::{self, File}, ops::Range, fmt::Display, collections::{HashMap}, hash::BuildHasherDefault,};
use itertools::{Itertools, repeat_n}; use indicatif::{ProgressBar, ProgressStyle, ProgressFinish}; use rustc_hash::{FxHashMap, FxHashSet, FxHasher};
use once_cell::sync::Lazy; use std::io::Write; use rayon::prelude::*; use serde::{Serialize, Deserialize};
#[cfg(test)] #[path = "./tests.rs"] mod tests;

/*------------------------------------------------------------
MAIN
-------------------------------------------------------------*/
fn main() {

    let mut app = App::new(default());
 
    app.bar.println("Calculating...");
    app.build_cache();

    app.bar.reset(); app.bar.reset_eta(); app.bar.set_draw_rate(1);
    app.bar.set_length(app.ev_cache.len() as u64);
    app.bar.println("Outputing...");

    print_state_choices_header();
    for entry in &app.ev_cache { 
        print_state_choice(entry.0, *entry.1); 
        app.bar.inc(1);
    }
    app.bar.println("Done!")

}

/*-------------------------------------------------------------
CONSTS etc
-------------------------------------------------------------*/

type Choice     = u8; // represents EITHER chosen scorecard Slot, OR a chosen dice Selection (below)
type Selection  = u8; // a bitfield representing a selection of dice to roll (1 means roll, 0 means don't)
type Slot       = u8; // a single scorecard slot with values ranging from ACES to CHANCE 
type DieVal     = u8; // a single die value 0 to 6 where 0 means "unselected"
type YahtCache  = HashMap::<GameState,ChoiceEV,BuildHasherDefault<FxHasher>>;

struct SlotID; impl SlotID{
    const STUB:Slot=0; const ACES:Slot=1; const TWOS:Slot=2; const THREES:Slot=3; const FOURS:Slot=4; const FIVES:Slot=5; const SIXES:Slot=6;
    const THREE_OF_A_KIND:Slot=7; const FOUR_OF_A_KIND:Slot=8; const FULL_HOUSE:Slot=9; const SM_STRAIGHT:Slot=10; const LG_STRAIGHT:Slot=11; 
    const YAHTZEE:Slot=12; const CHANCE:Slot=13; 
}
 
static SELECTION_RANGES:Lazy<[Range<usize>;32]> = Lazy::new(selection_ranges); 
static OUTCOMES:Lazy<[Outcome;1683]> = Lazy::new(all_selection_outcomes); 
static FACT:Lazy<[u64;21]> = Lazy::new(||{let mut a:[u64;21]=[0;21]; for i in 0..=20 {a[i]=fact(i as u8);} a});  // cached factorials
static DIEVALS_ID_FOR_DIEVALS:Lazy<[DieValsID;28087]> = Lazy::new(dievals_id_for_dievals); //the compact sorted version for every 5-dieval-permutation-with-repetition
static DIEVALS_FOR_DIEVALS_ID:Lazy<[DieVals;253]> = Lazy::new(dievals_for_dievals_id); 

/*-------------------------------------------------------------
APP
-------------------------------------------------------------*/
struct App{
    game: GameState,
    bar:ProgressBar,
    ev_cache:YahtCache,
}
impl App{

   /// return a newly initialized app
   fn new(game: GameState) -> Self{
        
        // precache some hot values
        let GameStateCounts{ lookups, saves} = game.counts();

        // init "UI"
        let bar = ProgressBar::new(lookups);
        bar.set_draw_rate(1);
        bar.set_style(ProgressStyle::default_bar()
            .template("{prefix} {wide_bar} {percent}% {pos:>4}/{len:4} {elapsed:>}/{duration} ETA:{eta}")
            // .on_finish(ProgressFinish::AtCurrentPos)
        );

        // prep cache 
        let init_hash_capacity = saves; 
        let ev_cache = if let Ok(bytes) = fs::read("ev_cache") { 
            ::bincode::deserialize(&bytes).unwrap() //read binary cache from disk if it happens to exist from use of app.save_cache()
        } else {
            YahtCache::with_capacity_and_hasher(init_hash_capacity, default())
        };

        Self{game, bar, ev_cache}
    }

    /*-------------------------------------------------------------
    BUILD_CACHE
    -------------------------------------------------------------*/

    /// gather up expected values in a multithreaded bottom-up fashion. this is like.. the main thing
    fn build_cache(&mut self) {
        // let sorted = INDEX_FOR_DIEVALS_SORTED.clone();
        let all_die_combos=outcomes_for_selection(0b11111);
        let placeholder_dievals= &OUTCOMES[0..=0]; //OUTCOMES[0] == [Dievals::default()]
        // let mut leaf_cache = [ChoiceEV;8888]  //TODO this could be a straight array . faster?
        let mut leaf_cache = YahtCache::default();

        // first handle special case of the most leafy leaf calcs -- where there's one slot left and no rolls remaining
        for single_slot in self.game.sorted_open_slots {  
            let slot:SortedSlots = [single_slot].into(); //set of a single slot 
            let joker_rules_in_play = single_slot!=SlotID::YAHTZEE; // joker rules in effect when the yahtzee slot is not open 
            for yahtzee_bonus_available in [false, joker_rules_in_play].to().unique() { // yahtzee bonus -might- be available when joker rules are in play 
                for upper_total in slot.relevant_upper_totals(){
                    for outcome in all_die_combos{
                        let state = GameState{
                            rolls_remaining: 0, 
                            sorted_dievals: outcome.dievals.into(), 
                            sorted_open_slots: slot, 
                            upper_total, 
                            yahtzee_bonus_avail: yahtzee_bonus_available,
                        };
                        let score = state.score_first_slot_in_context() as f32;
                        let choice_ev = ChoiceEV{ choice: single_slot, ev: score};
                        leaf_cache.insert(state, choice_ev);
                        self.output_state_choice(&state, choice_ev)
        } } } }

        // for each length 
        for slots_len in 1..=self.game.sorted_open_slots.len(){ 

            // for each slotset (of above length)
            for slots_vec in self.game.sorted_open_slots.to().combinations(slots_len as usize) {
                let slots:SortedSlots = slots_vec.into(); 
                let joker_rules_in_play = !slots.to().contains(&SlotID::YAHTZEE); // joker rules are in effect whenever the yahtzee slot is already filled 

                // for each upper total 
                for upper_total in slots.relevant_upper_totals() {

                    // for each yathzee bonu possibility 
                    for yahtzee_bonus_available in [false,joker_rules_in_play].to().unique() {// bonus always unavailable unless yahtzees are wild first

                        self.bar.inc(848484); // advance the progress bar by the number of cache reads coming up for dice selection 
                        self.bar.inc(252 * slots_len as u64 *if slots_len ==1{1}else{2}); // advance for slot selection cache reads

                        // for each rolls remaining
                        for rolls_remaining in [0,1,2,3] { 

                            let die_combos = if rolls_remaining==3 {placeholder_dievals} else {all_die_combos}; 

                            let built_from_threads = die_combos.into_par_iter().fold(YahtCache::default, |mut built_this_thread, die_combo|{  

                                if rolls_remaining==0  { 
                                /* HANDLE SLOT SELECTION */

                                    let mut slot_choice_ev:ChoiceEV = default();

                                    for slot in slots {

                                        //joker rules say extra yahtzees must be played in their matching upper slot if it's available
                                        let first_dieval =die_combo.dievals.get(0);
                                        let joker_rules_matter = joker_rules_in_play && Score::yahtzee(die_combo.dievals)>0 && slots.to().contains(&first_dieval);
                                        let head_slot:Slot = if joker_rules_matter { first_dieval } else { slot };
                                        let head:SortedSlots = [head_slot].into();

                                        let mut yahtzee_bonus_avail_now = yahtzee_bonus_available;
                                        let mut upper_total_now = upper_total;
                                        let mut dievals_or_wildcard = die_combo.dievals; 
                                        let tail = if slots_len > 1 { slots.removed(head_slot) } else {head};
                                        let mut head_plus_tail_ev = 0.0;
        
                                        // find the collective ev for the all the slots with this iteration's slot being first 
                                        // do this by summing the ev for the first (head) slot with the ev value that we look up for the remaining (tail) slots
                                        let mut rolls_remaining_now = 0;
                                        for slots_piece in [head,tail].to().unique(){
                                            let state = &GameState{
                                                rolls_remaining: rolls_remaining_now, 
                                                sorted_dievals: dievals_or_wildcard.into(),
                                                sorted_open_slots: slots_piece, 
                                                upper_total:  if slots_piece.best_upper_total() + upper_total_now >= 63 {upper_total_now} else {0},  
                                                yahtzee_bonus_avail: yahtzee_bonus_avail_now,
                                            };
                                            let cache = if slots_piece==head { &leaf_cache } else { &self.ev_cache}; //TODO why need leaf_cache separate from main? how is leaf_cache returning upper_total > 0?? also how is this shared state read from multi threads??
                                            let choice_ev = cache.get(state).unwrap(); 
                                            if slots_piece==head { // on the first pass only.. 
                                                //going into tail slots next, we may need to adjust the state based on the head choice
                                                if choice_ev.choice <=SlotID::SIXES { // adjust upper total for the next pass 
                                                    let added = (choice_ev.ev as u8) % 100; // the modulo 100 here removes any yathzee bonus from ev since that doesnt' count toward upper bonus total
                                                    upper_total_now = min(63, upper_total_now + added);
                                                } else if choice_ev.choice==SlotID::YAHTZEE { // adjust yahtzee related state for the next pass
                                                    if choice_ev.ev>0.0 {yahtzee_bonus_avail_now=true;};
                                                }
                                                rolls_remaining_now=3; // for upcoming tail lookup, we always want the ev for 3 rolls remaining
                                                dievals_or_wildcard = DieVals::default() // for 3 rolls remaining, use "wildcard" representative dievals since dice don't matter when rolling all of them
                                            }
                                            head_plus_tail_ev += choice_ev.ev;
                                        } //end for slot_piece
                                        if head_plus_tail_ev >= slot_choice_ev.ev { slot_choice_ev = ChoiceEV{ choice: slot, ev: head_plus_tail_ev}};
                                        
                                        if joker_rules_matter {break};// if joker-rules-matter we were forced to choose one slot, so we can skip trying the rest  
                                    }
                                    
                                    let state = GameState {
                                        sorted_dievals: die_combo.dievals.into(), 
                                        sorted_open_slots: slots,
                                        rolls_remaining: 0, 
                                        upper_total, 
                                        yahtzee_bonus_avail: yahtzee_bonus_available ,
                                    };
                                    built_this_thread.insert( state, slot_choice_ev);
                                    self.output_state_choice(&state, slot_choice_ev)

                                } else { //if rolls_remaining > 0  
                                /* HANDLE DICE SELECTION */    

                                    let next_roll = rolls_remaining-1; 
                                    let mut best_dice_choice_ev = ChoiceEV::default();
                                    let selections = if rolls_remaining ==3 { // selections are bitfields where '1' means roll and '0' means don't roll 
                                        0b11111..=0b11111 //always select all dice on the initial roll . 
                                    } else { 0b00000..=0b11111}; //otherwise try all selections
                                    for selection in selections { // we'll try each selection against this starting dice combo  
                                        let mut total_ev_for_selection = 0.0; 
                                        let mut outcomes_count:u64= 0; 
                                        for roll_outcome in outcomes_for_selection(selection) {
                                            let mut newvals = die_combo.dievals;
                                            newvals.blit(roll_outcome.dievals, roll_outcome.mask);
                                            // newvals = sorted[&newvals]; 
                                            let state = GameState{
                                                sorted_dievals: newvals.into(), 
                                                sorted_open_slots: slots, 
                                                upper_total:if slots.best_upper_total() + upper_total >= 63 {upper_total} else {0},   
                                                yahtzee_bonus_avail: yahtzee_bonus_available, 
                                                rolls_remaining: next_roll, // we'll average all the 'next roll' possibilities (which we'd calclated last) to get ev for 'this roll' 
                                            };
                                            let ev_for_this_selection_outcome = self.ev_cache.get(&state).unwrap().ev; 
                                            total_ev_for_selection += ev_for_this_selection_outcome * roll_outcome.arrangements as f32;// bake into upcoming average
                                            outcomes_count += roll_outcome.arrangements as u64; // we loop through die "combos" but we'll average all "perumtations"
                                        }
                                        let avg_ev_for_selection = total_ev_for_selection / outcomes_count as f32;
                                        if avg_ev_for_selection > best_dice_choice_ev.ev{
                                            best_dice_choice_ev = ChoiceEV{choice:selection as u8, ev:avg_ev_for_selection};
                                        }
                                    }
                                    let state = GameState{
                                            sorted_dievals: die_combo.dievals.into(),  
                                            sorted_open_slots: slots, 
                                            upper_total, 
                                            yahtzee_bonus_avail: yahtzee_bonus_available, 
                                            rolls_remaining, 
                                        }; 
                                    self.output_state_choice(&state, best_dice_choice_ev);
                                    built_this_thread.insert(state,best_dice_choice_ev);
    
                                } // endif roll_remaining...  

                                built_this_thread

                            }).reduce(YahtCache::default, |mut a,built_from_thread|{
                                a.extend(&built_from_thread); a 
                            }); // end die_combos.par_into_iter() 

                            self.ev_cache.extend(&built_from_threads);

                        } // end for each rolls_remaining
                    } //end for each yahtzee_is_wild
                } //end for each upper total 
            } // end for each slot_set 
        } // end for each length
    } // end fn build_cache

    fn best_choice_ev(&mut self) -> ChoiceEV{
        debug_assert!(!self.ev_cache.is_empty());
        *self.ev_cache.get(&self.game).unwrap()
    }

    fn save_cache(&self){
        let evs = &self.ev_cache; 
        let mut f = &File::create("ev_cache").unwrap();
        let bytes = bincode::serialize(evs).unwrap();
        f.write_all(&bytes).unwrap();
    }

    fn output_state_choice(&self, state: &GameState, choice_ev:ChoiceEV, ){
        // Uncomment below for more verbose progress output at the expense of speed 
        // if state.rolls_remaining==0 {
        //     self.bar.println(format!("S\t{: >6.2?}\t{:_^5}\t{:2?}\t{}\t{:2?}\t{}\t{: <29}",
        //         choice_ev.ev, choice_ev.choice, state.rolls_remaining, state.sorted_dievals, state.upper_total, 
        //         if state.yahtzee_bonus_avail {"Y"}else{""}, state.sorted_open_slots.to_string())); 
        // } else {
        //     self.bar.println(format!("D\t{: >6.2?}\t{:05b}\t{:2?}\t{}\t{:2?}\t{}\t{: <29}",
        //         choice_ev.ev, choice_ev.choice, state.rolls_remaining, state.sorted_dievals, state.upper_total, 
        //         if state.yahtzee_bonus_avail {"Y"}else{""}, state.sorted_open_slots.to_string())); 
        // };
    }


}

/*-------------------------------------------------------------
GameState
-------------------------------------------------------------*/
#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Clone, Copy, Serialize, Deserialize)]
struct GameState{
    sorted_dievals:DieValsID, //3bits per die unsorted =15 bits minimally ... 8bits if combo is stored sorted (252 possibilities)
    sorted_open_slots:SortedSlots, // 13 bits " 4 bits for a single slot 
    upper_total:u8, // 6 bits " 
    rolls_remaining:u8, // 3 bits "
    yahtzee_bonus_avail:bool, // 1 bit "
} //~500k for leafcalcs

impl GameState{ 
    /// calculate relevant counts for gamestate: required lookups and saves
    fn counts(self) -> GameStateCounts {

        let mut lookups:u64 = 0;
        let mut saves:usize =0;
        for subset_len in 1..=self.sorted_open_slots.len(){ 
            for slots_vec in self.sorted_open_slots.to().combinations(subset_len as usize) {
                let slots:SortedSlots =slots_vec.into(); 
                let joker_rules = !slots.to().contains(&SlotID::YAHTZEE); // yahtzees aren't wild whenever yahtzee slot is still available 
                for _upper_total in slots.relevant_upper_totals() {
                    for _yahtzee_bonus_avail in [false,joker_rules].to().unique() {
                        let slot_lookups = (subset_len as u64 * if subset_len==1{1}else{2} as u64) * 252 ;// * subset_len as u64;
                        let dice_lookups = 848484; // previoiusly verified by counting up by 1s in the actual loop. however chunking forward is faster 
                        lookups += dice_lookups + slot_lookups;
                        saves+=1;
        }}}}
        
        GameStateCounts{ lookups, saves } 
    }

    pub fn score_first_slot_in_context(&self) -> u8 {
    
        /* score slot itself w/o regard to game state */
            let slot = self.sorted_open_slots.to().next().unwrap();
            let mut score = Score::slot_with_dice(slot, self.sorted_dievals.into()); 
    
        /* add upper bonus when needed total is reached */
            if slot<=SlotID::SIXES && self.upper_total>0 { 
                let new_upper_total = min(self.upper_total+score, 63) ;
                if new_upper_total==63 {score += 35}; 
            } 
    
        /* special handling of "joker rules" */
            let just_rolled_yahtzee = Score::yahtzee(self.sorted_dievals.into())==50;
            let joker_rules_in_play = slot!=SlotID::YAHTZEE; // joker rules in effect when the yahtzee slot is not open 
            if just_rolled_yahtzee && joker_rules_in_play{ // standard scoring applies against the yahtzee dice except ... 
                if slot==SlotID::FULL_HOUSE  {score=25}; 
                if slot==SlotID::SM_STRAIGHT {score=30}; 
                if slot==SlotID::LG_STRAIGHT {score=40}; 
            }
    
        /* special handling of "extra yahtzee" bonus per rules*/
            if just_rolled_yahtzee && self.yahtzee_bonus_avail { 
                score+=100; // extra yahtzee bonus per rules
            }
    
        score
    }

}

impl Default for GameState{
    fn default() -> Self {
        Self { sorted_dievals: default(), rolls_remaining: 3, upper_total: 63, 
            yahtzee_bonus_avail: false, sorted_open_slots: [1,2,3,4,5,6,7,8,9,10,11,12,13].into(),
        }
    }
}

#[derive(Debug)]
struct GameStateCounts {
    lookups:u64,
    saves:usize 
}


/*-------------------------------------------------------------
INITIALIZERS
-------------------------------------------------------------*/

fn dievals_id_for_dievals() -> [DieValsID;28087] {
    let mut arr=[DieValsID{data:0};28087];
    arr[0] = DieValsID { data: 0};// first one is the special wildcard 
    for (i,combo) in (1u8..=6).combinations_with_replacement(5).enumerate() {
        for perm in combo.to().permutations(5).unique(){
            let dievals:DieVals = perm.clone().to().collect_vec().into();
            arr[dievals.data as usize]= DieValsID { data: i as u8 + 1} ;
        }
    };
    arr
}

fn dievals_for_dievals_id() -> [DieVals; 253] {
    let mut out=[DieVals::default(); 253];
    out[0]=[0,0,0,0,0].into(); // first one is the special wildcard 
    for (i,combo) in (1u8..=6).combinations_with_replacement(5).enumerate() {
        out[i+1]=combo.into();
    }
    out
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

/// returns a slice from the precomputed dice roll outcomes that corresponds to the given selection bitfield 
fn outcomes_for_selection(selection:u8)->&'static [Outcome]{
    const IDX_FOR_SELECTION:[usize;32] = [0,1,2,3,4,7,6,16,8,9,10,17,11,13,19,26,5,12,18,20,14,21,22,23,15,25,24,27,28,29,30,31];
    let idx = IDX_FOR_SELECTION[selection as usize];
    let range = SELECTION_RANGES[idx].clone();
    &OUTCOMES[range]
}


/*-------------------------------------------------------------
SortedSlots
-------------------------------------------------------------*/

// the following LLDB command will format SortedSlots with meaningful values in the debugger 
// type summary add --summary-string "${var.data%b}" "yahtzeebot::SortedSlots"

#[derive(Debug,Clone,Copy,PartialEq,Serialize,Deserialize,Eq,PartialOrd,Ord,Hash,Default)]

struct SortedSlots{
    pub data: u16, // 13 sorted Slots can be positionally encoded in one u16
}

impl SortedSlots{
    fn len(self)->u8{
        16-self.data.leading_zeros() as u8
    }
    fn insert (&mut self, val:Slot){
        let mask = 1<<val;
        self.data |= mask; // force on
    }
    fn remove (&mut self, val:Slot){
        let mask = !(1<<val);
        self.data &= mask; //force off
    }
    fn removed(self, val:Slot) -> Self{
        let mut out = self;
        out.remove(val);
        out
    }
    fn has (self, val:Slot) -> bool {
        self.data & (1<<val) > 0  
    }
    fn previously_used_upper_slots(self) -> Self{ 
        let mut out:Self= self;
        out.data = (!out.data) & ((1<<7)-1);
        out
    }
 
    /// returns the unique and relevant "upper bonus total" that could have occurred from the previously used upper slots 
    fn relevant_upper_totals(self) -> impl Iterator<Item=u8>   {  
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
        let used_slot_idxs = &self.previously_used_upper_slots().to().filter(|&x|x<=SlotID::SIXES).map(|x| x as usize).collect_vec(); // TODO needless double filtered
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
        // NOTE this filters out a lot of unneeded state space but means the lookup function must map extraneous deficits to a default 
        let best_current_slot_total = self.best_upper_total();
        totals.to().filter/*keep!*/(move |used_slots_total| 
            *used_slots_total==0 || // always relevant 
            *used_slots_total + best_current_slot_total >= 63 // totals must reach the bonus threshhold to be relevant
        )
    }

   //converts the given total to a default if the bonus threshold can't be reached 
   fn relevant_total(self,given_total:u8) -> u8{
    if self.best_upper_total() + given_total >= 63 {given_total} else {0}
}

    fn best_upper_total (self) -> u8{
        let mut sum=0;
        for x in self { if x>6 {break} else {sum+=x;} }
        sum*5
    }

}

impl Display for SortedSlots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to().for_each(|x| write!(f,"{}_",x).unwrap());
        Ok(())
    }
}

impl <const N:usize> From<[Slot; N]> for SortedSlots{
    fn from(a: [Slot; N]) -> Self {
        assert! (a.len() <= 13);
        let mut retval:Self = default();
        for x in a { retval.insert(x); }
        retval 
    }
}
impl From<Vec<Slot>> for SortedSlots{
    fn from(v: Vec<Slot>) -> Self {
        assert! (v.len() <= 13);
        let mut retval:Self = default();
        for x in v { retval.insert(x); }
        retval 
    }
}


impl IntoIterator for SortedSlots{
    type IntoIter=SortedSlotsIntoIter;
    type Item = Slot;

    fn into_iter(self) -> Self::IntoIter {
        SortedSlotsIntoIter { sorted_slots:self, i:0 }
    }
}

struct SortedSlotsIntoIter{
    sorted_slots: SortedSlots,
    i: u8,
}

impl Iterator for SortedSlotsIntoIter {
    type Item = Slot ;
    fn next(&mut self) -> Option<Self::Item> {
        while self.i < 13 {
            self.i+=1;
            if self.sorted_slots.has(self.i) { return Some(self.i) }
        }
        None
    }
}

/*-------------------------------------------------------------
DieVals
-------------------------------------------------------------*/

#[derive(Debug,Clone,Copy,PartialEq,Serialize,Deserialize,Eq,PartialOrd,Ord,Hash,Default)]

struct DieVals{
    data:u16, // 5 dievals, each from 0 to 6, can be encoded in 2 bytes total, each taking 3 bits
}

// the following LLDB command will format DieVals with meaningful values in the debugger 
//    type summary add --summary-string "${var.data[0-2]%u} ${var.data[3-5]%u} ${var.data[6-8]%u} ${var.data[9-11]%u} ${var.data[12-14]%u}" "yahtzeebot::DieVals"

impl DieVals {

    fn set(&mut self, index:u8, val:DieVal) { 
        let bitpos = 3*index; // widths of 3 bits per value
        let mask = ! (0b111_u16 << bitpos); // hole maker
        self.data = (self.data & mask) | ((val as u16) << bitpos ); // punch & fill hole
    }

    /// blit the 'from' dievals into the 'self' dievals with the help of a mask where 0 indicates incoming 'from' bits and 1 indicates none incoming 
    fn blit(&mut self, from:DieVals, mask:DieVals,){
        self.data = (self.data & mask.data) | from.data;//TODO mask actually needed?
    }

    fn get(&self, index:u8)->DieVal{
        ((self.data >> (index*3)) & 0b111) as DieVal
    }

}

impl Display for DieVals { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f,"{}{}{}{}{}",self.get(4), self.get(3),self.get(2),self.get(1),self.get(0)) 
}}

impl From<Vec<DieVal>> for DieVals{ fn from(v: Vec<DieVal>) -> Self {
    let mut a:[DieVal;5]=default();
    a.copy_from_slice(&v[0..5]);
    a.into()
}}

impl From<[DieVal; 5]> for DieVals{ fn from(a: [DieVal; 5]) -> DieVals{
    DieVals{ data: (a[4] as u16) << 12 | (a[3] as u16) <<9 | (a[2] as u16) <<6 | (a[1] as u16) <<3 | (a[0] as u16),}
}}

impl From<& DieVals> for [DieVal; 5]{ fn from(dievals: &DieVals) -> [DieVal; 5] {
    let mut temp:[DieVal;5] = default(); 
    for i in 0_u8..=4 {temp[i as usize] = dievals.get(i)};
    temp
}}

impl From<DieVals> for [DieVal; 5]{ fn from(dievals: DieVals) -> [DieVal; 5] {
    <[DieVal;5]>::from(&dievals)
}}

impl From<&mut DieVals> for [DieVal; 5]{ fn from(dievals: &mut DieVals) -> [DieVal;5]{
    <[DieVal;5]>::from(&*dievals)
}}

impl IntoIterator for DieVals{ 
    type IntoIter=DieValsIntoIter; type Item = DieVal; 
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
DiesValsID
-------------------------------------------------------------*/
#[derive(Debug,Clone,Copy,PartialEq,Serialize,Deserialize,Eq,PartialOrd,Ord,Hash,Default)]

struct DieValsID{
    data:u8, // all 252 sorted dievals combos can be encoded in 8 bits using their index/id
}
impl From<DieValsID> for DieVals{ fn from(sorted_dievals:DieValsID) -> DieVals{
    DIEVALS_FOR_DIEVALS_ID[sorted_dievals.data as usize]
}}
impl From<DieVals> for DieValsID{ fn from(dievals:DieVals) -> DieValsID{
    DIEVALS_ID_FOR_DIEVALS[dievals.data as usize]
}}
impl From<[u8;5]> for DieValsID{ fn from(a:[u8;5]) -> DieValsID{
    DieVals::from(a).into()
}}
impl Display for DieValsID { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    DieVals::from(*self).fmt(f)
}}



/*-------------------------------------------------------------
SCORING FNs
-------------------------------------------------------------*/
struct Score; impl Score { 

    const SCORE_FNS:[fn(sorted_dievals:DieVals)->u8;14] = [
        |_x|panic!(), // stub so indices align more intuitively with categories 
        Score::aces, Score::twos, Score::threes, Score::fours, Score::fives, Score::sixes, 
        Score::three_of_a_kind, Score::four_of_a_kind, Score::fullhouse, Score::sm_str8, Score::lg_str8, Score::yahtzee, Score::chance, 
    ];

    fn upperbox(boxnum:Slot, sorted_dievals:DieVals)->u8{
        sorted_dievals.to().filter(|x| *x==boxnum).sum()
    }
    
    fn n_of_a_kind(n:u8,sorted_dievals:DieVals)->u8{
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
    
    fn aces(sorted_dievals:       DieVals)->u8 { Score::upperbox(1,sorted_dievals) }
    fn twos(sorted_dievals:       DieVals)->u8 { Score::upperbox(2,sorted_dievals) }
    fn threes(sorted_dievals:     DieVals)->u8 { Score::upperbox(3,sorted_dievals) }
    fn fours(sorted_dievals:      DieVals)->u8 { Score::upperbox(4,sorted_dievals) }
    fn fives(sorted_dievals:      DieVals)->u8 { Score::upperbox(5,sorted_dievals) }
    fn sixes(sorted_dievals:      DieVals)->u8 { Score::upperbox(6,sorted_dievals) }
    
    fn three_of_a_kind(sorted_dievals:   DieVals)->u8 { Score::n_of_a_kind(3,sorted_dievals) }
    fn four_of_a_kind(sorted_dievals:   DieVals)->u8 { Score::n_of_a_kind(4,sorted_dievals) }
    fn sm_str8(sorted_dievals:    DieVals)->u8 { if Score::straight_len(sorted_dievals) >=4 {30} else {0} }
    fn lg_str8(sorted_dievals:    DieVals)->u8 { if Score::straight_len(sorted_dievals) ==5 {40} else {0} }
        // if sorted_dievals==[1,2,3,4,5].into() || sorted_dievals==[2,3,4,5,6].into() {40} else {0} }
    
    // The official rule is that a Full House is "three of one number and two of another"
    fn fullhouse(sorted_dievals:DieVals) -> u8 { 
        let counts = sorted_dievals.to().counts();
        if counts.len() != 2 {return 0};
        let mut it = counts.into_iter(); 
        let &(val1,val1count) = &it.next().unwrap();
        let (val2,val2count) = it.next().unwrap(); 
        if val1==0 || val2==0 {return 0};
        if (val1count==3 && val2count==2) || (val2count==3 && val1count==2) {25} else {0}
    }
    
    fn chance(sorted_dievals:DieVals)->u8 { sorted_dievals.to().sum()  }
    
    fn yahtzee(sorted_dievals:DieVals)->u8 { 
        if sorted_dievals.get(0) == sorted_dievals.get(4) && sorted_dievals.get(0) != 0 {50} else {0}
    }
       
    /// reports the score for a set of dice in a given slot w/o regard for exogenous gamestate (bonuses, yahtzee wildcards etc)
    fn slot_with_dice(slot:Slot, sorted_dievals:DieVals)->u8{
        Score::SCORE_FNS[slot as usize](sorted_dievals) 
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
UTILS
-------------------------------------------------------------*/
/// allow use of to() where into_iter() is normally required. so wrong but so right. 
trait IntoIterShortcut {
    type Item;
    type IntoIter: Iterator<Item = Self::Item>;
    fn to(self) -> Self::IntoIter; 
}
impl<T: IntoIterator> IntoIterShortcut for T{ 
    type Item=T::Item;
    type IntoIter= T::IntoIter;//Iterator<Item = T::Item>;
    fn to(self) -> Self::IntoIter { self.into_iter() }
}

/// my own default_free_fn
#[inline] pub fn default<T: Default>() -> T {
   Default::default() 
}

/// rudimentary factorial suitable for our purposes here.. handles up to fact(20) 
fn fact(n: u8) -> u64{
    if n<=1 {1} else { (n as u64)*fact(n-1) }
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

fn print_state_choices_header(){
    println!("choice_type,choice,dice,rolls_remaining,upper_total,yahtzee_bonus_avail,open_slots,expected_value");
}

fn print_state_choice(state: &GameState, choice_ev:ChoiceEV){
    if state.rolls_remaining==0 {
        println!("S,{},{},{},{},{},{},{}",
            choice_ev.choice, state.sorted_dievals, state.rolls_remaining, state.upper_total, 
            if state.yahtzee_bonus_avail {"Y"}else{""}, state.sorted_open_slots, choice_ev.ev); 
    } else {
        println!("D,{:05b},{},{},{},{},{},{}",
            choice_ev.choice, state.sorted_dievals, state.rolls_remaining, state.upper_total, 
            if state.yahtzee_bonus_avail {"Y"}else{""}, state.sorted_open_slots, choice_ev.ev); 
    };
}
