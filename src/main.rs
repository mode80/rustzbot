#![allow(dead_code)]
//#![allow(unused_variables)]
#![allow(unused_imports)]

use std::{cmp::max, sync::{Arc, RwLock, Mutex}, error::{self, Error}, fs::{self, File}, time::Duration};
// use cached::proc_macro::cached;
use counter::Counter;
use itertools::Itertools;
use indicatif::ProgressBar;
use rustc_hash::{FxHashSet, FxHashMap};
use tinyvec::*;
use rayon::prelude::*; 
use once_cell::sync::Lazy;
use std::io::Write; 

#[macro_use] extern crate serde_derive;
extern crate bincode;

#[cfg(test)] 
#[path = "./tests.rs"]
mod tests;
//-------------------------------------------------------------*/

fn main() -> Result<(), Box<dyn Error>>{
    
    let game = GameState{
        sorted_open_slots: ArrayVec::from([ACES,TWOS,THREES,FOURS,FIVES,SIXES,THREE_OF_A_KIND,FOUR_OF_A_KIND,SM_STRAIGHT,LG_STRAIGHT,FULL_HOUSE,YAHTZEE,CHANCE]),
        sorted_dievals: UNROLLED_DIEVALS, rolls_remaining: 3, upper_bonus_deficit: INIT_DEFICIT, yahtzee_is_wild: false,
    };

    let app = & mut AppState::new(&game);

    let (_choice, _ev) = best_choice_ev(game, app);
   
    Ok(())
   
}
/*-------------------------------------------------------------*/

#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Clone, Copy, Serialize, Deserialize)]
struct GameState{
    sorted_dievals:[u8;5], 
    rolls_remaining:u8, 
    upper_bonus_deficit:u8, 
    yahtzee_is_wild:bool,
    sorted_open_slots:ArrayVec<[u8;13]>, 
}

#[derive(Debug,Clone,Copy,Serialize,Deserialize,PartialEq)]
enum Choice{
    Slot(u8),
    Dice(ArrayVec<[u8;5]>)
}

struct AppState{
    progress_bar:Arc<RwLock<ProgressBar>>, 
    done:Arc<RwLock<FxHashSet<ArrayVec<[u8;13]>>>>, 
    ev_cache:Arc<Mutex<FxHashMap<GameState,(Choice,f32)>>>,
    checkpoint: Arc<RwLock<Duration>>,
    // log, 
}
impl AppState{
    fn new(game: &GameState) -> Self{
        let slot_count=game.sorted_open_slots.len();
        let combo_count = (1..=slot_count).map(|r| n_take_r(slot_count as u128, r as u128,false,false) as u64 ).sum() ;
        // let bytes = fs::read("ev_cache").unwrap();
        // let cachemap = ::bincode::deserialize(&bytes).unwrap() ;
        let init_capacity = 1; // TODO experiment with larger values
        // let init_capacity = combo_count as usize * 252 * 64 * 2 * 2; // roughly: slotcombos * diecombos * deficits * wilds * rolls
        let cachemap = if let Ok(bytes) = fs::read("ev_cache") { 
            ::bincode::deserialize(&bytes).unwrap() 
        } else {
            FxHashMap::with_capacity_and_hasher(init_capacity,Default::default())
        };
        Self{   progress_bar : Arc::new(RwLock::new(ProgressBar::new(combo_count))), 
                done : Arc::new(RwLock::new(FxHashSet::default())) ,  
                ev_cache : Arc::new(Mutex::new(cachemap)),
                checkpoint: Arc::new(RwLock::new(Duration::new(0,0))),
        }
    }
}

const STUB:u8=0; const ACES:u8=1; const TWOS:u8=2; const THREES:u8=3; const FOURS:u8=4; const FIVES:u8=5; const SIXES:u8=6;
const THREE_OF_A_KIND:u8=7; const FOUR_OF_A_KIND:u8=8; const FULL_HOUSE:u8=9; const SM_STRAIGHT:u8=10; const LG_STRAIGHT:u8=11; 
const YAHTZEE:u8=12; const CHANCE:u8=13; 
 
const UNROLLED_DIEVALS:[u8;5] = [255,245,235,225,215]; const SIDES:u8 = 6; const INIT_DEFICIT:u8 = 63;

const SCORE_FNS:[fn(sorted_dievals:[u8;5])->u8;14] = [
    score_aces, // duplicate placeholder so indices align more intuitively with categories 
    score_aces, score_twos, score_threes, score_fours, score_fives, score_sixes, 
    score_3ofakind, score_4ofakind, score_fullhouse, score_sm_str8, score_lg_str8, score_yahtzee, score_chance, 
];

static OUTCOMES:Lazy<[[u8;5];7776]> = Lazy::new(all_outcomes_rolling_5_dice);
static SELECTIONS:Lazy<[ArrayVec<[u8;5]>;32]> = Lazy::new(die_index_combos); 
// [(), (0,), (0, 1), (0, 1, 2), (0, 1, 2, 3), (0, 1, 2, 3, 4), (0, 1, 2, 4), (0, 1, 3), (0, 1, 3, 4), 
// (0, 1, 4), (0, 2), (0, 2, 3), (0, 2, 3, 4), (0, 2, 4), (0, 3), (0, 3, 4), (0, 4), (1,), (1, 2), (1, 2, 3), (1, 2, 3, 4), 
// (1, 2, 4), (1, 3), (1, 3, 4), (1, 4), (2,), (2, 3), (2, 3, 4), (2, 4), (3,), (3, 4), (4,)]

/*-------------------------------------------------------------*/

/// rudimentary factorial suitable for our purposes here.. handles up to fact[34) */
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

fn save_periodically(app:&AppState, every_n_secs:u64){
    let current_duration = app.progress_bar.read().unwrap().elapsed();
    let last_duration = *app.checkpoint.read().unwrap();
    if current_duration - last_duration >= Duration::new(every_n_secs,0) { 
        save_cache(app);
   }
}

fn save_cache(app:&AppState){
    let evs = &*app.ev_cache.lock().unwrap(); // the "*" derefs the Arc smart pointer, while "&" makes it into the borrow we need 
    let mut f = &File::create("ev_cache").unwrap();
    let bytes = bincode::serialize(evs).unwrap();
    f.write_all(&bytes).unwrap();
}
 
fn console_log(game:&GameState, app:&AppState, choice:Choice, ev:f32 ){
    app.progress_bar.read().unwrap().println (
        format!("{:>}\t{}\t{:>4}\t{:>4.2}\t{:?}\t{:?}\t{:?}", 
            game.rolls_remaining, game.yahtzee_is_wild, game.upper_bonus_deficit, ev, game.sorted_dievals, game.sorted_open_slots, choice 
        )
    );
}
/*-------------------------------------------------------------*/

/// the set of all ways to roll different dice, as represented by a collection of indice vecs 
#[allow(clippy::eval_order_dependence)]
fn die_index_combos() ->[ArrayVec<[u8;5]>;32]  { 
    let mut i=0;
    let mut them:[ArrayVec<[u8;5]>;32] = [ArrayVec::<[u8;5]>::new() ;32]; // this is the empty selection
    for combo in (0..=4).combinations(1){ them[i]= {let mut it=ArrayVec::<[u8;5]>::new(); it.extend_from_slice(&combo); i+=1; it} } 
    for combo in (0..=4).combinations(2){ them[i]= {let mut it=ArrayVec::<[u8;5]>::new(); it.extend_from_slice(&combo); i+=1; it} } 
    for combo in (0..=4).combinations(3){ them[i]= {let mut it=ArrayVec::<[u8;5]>::new(); it.extend_from_slice(&combo); i+=1; it} } 
    for combo in (0..=4).combinations(4){ them[i]= {let mut it=ArrayVec::<[u8;5]>::new(); it.extend_from_slice(&combo); i+=1; it} } 
    for combo in (0..=4).combinations(5){ them[i]= {let mut it=ArrayVec::<[u8;5]>::new(); it.extend_from_slice(&combo); i+=1; it} } 
    them.sort_unstable();
    them
}

fn all_outcomes_rolling_5_dice() -> [[u8;5];7776] {

    let mut j:usize=0;
    let mut them:[[u8;5];7776] = [[0;5];7776]; 
    for i in 1..=6 {
        for ii in 1..=6 {
            for iii in 1..=6 {
                for iv in 1..=6 {
                    for v in 1..=6 {
                        them[j] = [i as u8, ii as u8, iii as u8, iv as u8, v as u8];
                        j+=1;
    } } } } }
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
        if x==lastval+1 {inarow+=1}
        else if x!=lastval {inarow=1};
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
fn score_sm_str8(sorted_dievals:    [u8;5])->u8{ if straight_len(sorted_dievals) >=4 {30} else {0} }
fn score_lg_str8(sorted_dievals:    [u8;5])->u8{ if straight_len(sorted_dievals) >=5 {40} else {0} }

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
fn score_slot(slot:u8, sorted_dievals:[u8;5])->u8{
    SCORE_FNS[slot as usize](sorted_dievals) 
}
/*-------------------------------------------------------------*/

/// returns the best slot and corresponding ev for final dice, given the slot possibilities and other relevant state 
fn best_slot_ev(game:GameState, app: &AppState) -> (Choice,f32) {

    let slot_sequences = game.sorted_open_slots.into_iter().permutations(game.sorted_open_slots.len()); // TODO make a version of this that returns ArrayVecs 
    let mut best_ev = 0.0; 
    let mut best_slot=STUB; 

    for slot_sequence_vec in slot_sequences {

        // LEAF CALCS 
            // prep vars
                let mut tail_ev = 0.0;
                let mut slot_sequence = ArrayVec::<[u8;13]>::new();
                slot_sequence.extend_from_slice(&slot_sequence_vec);
                let top_slot = slot_sequence.pop().unwrap();
                let mut _choice:Choice = Choice::Slot(top_slot);
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
                let yahtzee_rolled = game.sorted_dievals[0]==game.sorted_dievals[4]; 
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
                (_choice, tail_ev) = best_choice_ev( GameState{
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

    // console_log(game, app, Choice::Slot(best_slot), best_ev );
    (Choice::Slot(best_slot), best_ev)
}

/// returns the best selection of dice and corresponding ev, given slots left, existing dice, and other relevant state 
fn best_dice_ev(game:GameState, app: &AppState) -> (Choice,f32){ 

    let mut best_selection = array_vec![0,1,2,3,4]; // default selection is "all dice"
    let mut best_ev = 0.0; 
    if game.rolls_remaining==3 {// special case .. we always roll all dice on initial roll
        best_ev = avg_ev_for_selection(game,app,best_selection);
        return (Choice::Dice(best_selection), best_ev)
    } else { // iterate over all the possible ways to select dice and take the best outcome 
        for selection in SELECTIONS.into_iter() {
            let avg_ev = avg_ev_for_selection(game,app,selection);
            if avg_ev > best_ev {best_ev = avg_ev; best_selection = selection; }
        }
    }
    // console_log(game, app, Choice::Dice(best_selection), best_ev );
    (Choice::Dice(best_selection), best_ev)
}

/// returns the average of all the expected values for rolling a selection of dice, given the game and app state
/// "selection" is the set of dice to roll, as represented their indexes in a 5-length array
#[inline(always)] // ~6% speedup 
fn avg_ev_for_selection(game:GameState, app: &AppState, selection:ArrayVec::<[u8;5]>) -> f32 {
    let selection_len = selection.len(); // this is how many dice we're selecting to roll
    // optimization: we'll always iterate over (some amount) of the outcomes of rolling 5 dice . This works because
    // the trailing 'n' dice from this set amount to the same set outcomes for when 'n' diced are selected 
    let idx_offset = 5-selection_len; // this will be the offset into the corrrect position when 'n' diced are selected. 
    let outcomes_count = [1,6,36,216,1296,7776][selection_len]; // we've pre-calcuated how many outcomes we need to iterate over
    let mut total = 0.0;
    let mut newvals:[u8;5]; 
    for outcome in OUTCOMES.iter().take(outcomes_count) { 
        //###### HOT CODE PATH #######
        newvals=game.sorted_dievals;
        for (i, j) in selection.into_iter().enumerate() { // TODO bitmask math as an optimization 
            newvals[j as usize]=outcome[i+idx_offset];    
        }
        newvals.sort_unstable();
        let (_choice, next_ev) = best_choice_ev( GameState{ 
            yahtzee_is_wild: game.yahtzee_is_wild, 
            sorted_open_slots: game.sorted_open_slots, 
            rolls_remaining: game.rolls_remaining-1,
            upper_bonus_deficit: game.upper_bonus_deficit,
            sorted_dievals: newvals, 
        }, app);
        total += next_ev 
        //############################
    }
    total/outcomes_count as f32
}


/// returns the best game Choice along with its expected value, given relevant game state.
// #[cached(key = "GameState", convert = r#"{ game }"#)] 
fn best_choice_ev(game:GameState,app: &AppState) -> (Choice,f32) { 

    if let Some(result) = app.ev_cache.lock().unwrap().get(&game) { return *result}; // return cached result if we have one 

    let result = if game.rolls_remaining == 0 {
        best_slot_ev(game,app)  // <-----------------
    } else { 
        best_dice_ev(game,app)  // <-----------------
    };

    // console_log(&game,app,result.0,result.1);

    if game.rolls_remaining==0 { // periodically update progress and save
        let e = {app.done.read().unwrap().contains(&game.sorted_open_slots)} ;
        if ! e  {
            app.done.write().unwrap().insert(game.sorted_open_slots);
            app.progress_bar.write().unwrap().inc(1);
            console_log(&game,app,result.0,result.1);
            save_periodically(app,600) ;
        }
    }
    
    app.ev_cache.lock().unwrap().insert(game, result);
    result 
}

