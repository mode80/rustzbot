# yahtzeebot

Calculates and outputs the optimal player decision for all game states in solo-player Yahtzee. 

## Basic Usage

To generate a .csv file of the results:
>yahtzeebot > results.csv

This will produce a file with the first few lines resembling:

```
choice_type,choice,dice,rolls_remaining,upper_total,yahtzee_bonus_avail,open_slots,expected_value
D,01111,65411,2,31,Y,1_3_4_6_7_8_11_,119.23471
S,13,66641,0,11,Y,3_4_5_9_10_13_,113.45208
```


Each line represents a choice to be made, along with the correct decision.

- <b>choice_type</b> is 'S' when the choice is regarding which scorecard slot to use, and 'D' when the choice is about which dice to roll. 

- <b>choice</b> is the optimal choice to be made. When the choice_type is 'S' this will be the slot number to use. The number corresponds the slot's ordinal position on the scorecard. e.g. 1 is ACES, and 13 is CHANCE. When the choice_type is 'D', choice show will which dice to roll in the form of 5 binary digits. A value of 01000 would mean to roll the 2nd die only. A value of 00111 would mean to roll the last 3 dice. 

- <b>dice</b> are the current state of the dice values after the previous roll. 66644 means there are 3 dice showing six and 2 dice showing four. 

- <b>upper_total</b> is the current total of upper section scorecard slots. This is relevant for calculating the possibility of an upper total bonus.

- <b>yahtzee_bonus_avail</b> is 'Y' when a yahtzee has already been scored and therefore another yahtzee rolled would give a 100 point bonus.

- <b>open_slots</b> is the list of scorecard slots that are currently open and available

- <b>expected_value</b> is the calculation of how many points will be scored from this point forward in a game where the player makes the recommended choice and continues playing optimally.


A bot (or other player) making use of this data would assess the state of the game at each decision point, then find the row containing the matching dice, rolls remaining, upper section total, yahtzee bonus availability and open slots. That row would indicate the optimal choice and the expected value of points to be earned from optimal play going forward.  
