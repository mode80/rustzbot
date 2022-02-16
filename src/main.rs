#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

use itertools::Itertools;
use itertools::iproduct;


fn main() {
    let dievals: [u8; 5] = [0, 0, 0, 0, 0];
    let die_combo: [bool; 5] = [true, true, true, true, true];

    let die_combos = die_combos();

    println!("{}",fact(13));
    println!("{}",n_take_r(13,13,true,false));
    // let roll_outcomes = all_outcomes_for_rolling_n_dice(5);

    println!("hello yahtzeebot");
}

// rudimentary factorial suitable for our purposes here.. ie combos up to fact(13) */
fn fact(n: u64) -> u64{
    if n<=1 {1} else { n*fact(n-1) }
}

// count of arrangements that can be formed from r selections, chosen from n items, 
// where order DOES or DOESNT matter, and WITH or WITHOUT replacement, as specified
fn n_take_r(n:u64, r:u64, ordered:bool, with_replacement:bool)->u64{

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




// fn all_outcomes_for_rolling_n_dice(n:usize)  {
//     let mut them = [[0;5];usize::pow(2,3)];
//     for (j, (i, ii, iii, iv, v)) in iproduct!(1..=6, 1..=6, 1..=6, 1..=6, 1..=6).enumerate() {
//         them[j]=[i,ii,iii,iv,v];
//     }
//     return them;
// }

