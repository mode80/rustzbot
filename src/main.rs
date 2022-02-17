#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

use std::{collections::HashSet, vec, cmp::max};

use cached::proc_macro::cached;
use itertools::iproduct;


fn main() {
    //ad hoc testing code here for now 

    let dievals: [u8; 5] = [0, 0, 0, 0, 0];
    let die_combo: [bool; 5] = [true, true, true, true, true];

    let die_combos = die_combos();
    let die_index_combos = die_index_combos();

    // let it = score_upperbox(5, [1,4,2,5,5]) ;
    // let it = score_n_of_a_kind(3, [1,2,5,5,5]) ;
    let it = straight_len([1,2,3,4,6]);

    // let it = all_outcomes_for_rolling_n_dice(5);
    // println!("{}",fact(34));
    // println!("{}",n_take_r(13,13,true,false));
    // let roll_outcomes = all_outcomes_for_rolling_n_dice(5);

    // println!("{:#?}",die_index_combos.len());
    // let it:Vec<u128> = (1..=13).map(|r| n_take_r(13,r,false,false) ).collect::<>();

    println!("{:#?}", it);
}

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
            n.pow(r.try_into().unwrap()) 
        } else { // no replacement
            fact(n) / fact(n-r)
        }
    }
}

// the set of all ways to roll different dice, as represented by a collection of bool arrays
// [[0,0,0,0,0], [1,0,0,0,0], [0,1,0,0,0], [0,0,1,0,0], [0,0,0,1,0], [0,0,0,0,1], [1,1,0,0,0],...
fn die_combos()-> [[bool;5];64] {

    let mut them = [[false;5];64] ;
    let mut j = 0;
    let false_true = [false, true];
    for i in false_true {
        for ii in false_true {
            for iii in false_true {
                for iv in false_true {
                    for v in false_true {
                        them[j] = [i,ii,iii,iv,v]; 
                        j+=1;
                    }
                }
            }
        }
    }
    them
}


// // the set of all ways to roll different dice, as represented by a collection of indice vecs 
#[cached]
fn die_index_combos() -> Vec<Vec<u8>>  {

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

enum Slot {
    Aces, Twos, Threes, Fours, Fives, Sixes,
    ThreeOfAKind, FourOfAKind, 
    SmStraight, LgStraight, 
    FullHouse, Yahtzee, Chance 
}

const ALL_DICE:[u8;5] = [0,1,2,3,4];
const UNROLLED_DIEVALS:[u8;5] = [0,0,0,0,0];
const SIDES:u8 = 6;




