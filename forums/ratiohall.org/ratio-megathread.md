# Ratio megathread: the numbers, and how to use them
board: Reference   date: Y126-04-05   author: ratio_warden   status: open

- ratio_warden | Y126-04-05 | This thread is the reference. It is not a place to ask "is my line good". It is a place to put a number, say where the number came from, and let somebody check it.
  **The rule.** Cite the recipe. A number without a recipe is a rumour. If you say "one molder per furnace", write the recipe that gives you that.
  **What we know, from the machine descriptions:**
  - Furnace, copper: one copper ore to one molten copper, five seconds. Iron: three iron ore to two molten iron, ten seconds.
  - Molder: one molten metal to one ingot, two and a half seconds.
  - Press: one ingot to one plate, two and a half seconds. Two plate to one pipe. One plate to two wire, five seconds.
  - Coke oven: two coal to one coke and one waste, two seconds. A furnace coking: three coal to one coke, five seconds, no fuel.
  - Blast furnace, steel from coke: two iron plate and one coke to one steel and one waste, ten seconds.
  That is enough to answer most of the questions that come here. Post your arithmetic. Show the division.
  - table_herr | Y126-04-05 | A drawn version, for those who read better with lines:
    ore --5s--> metal --2.5s--> ingot --2.5s--> plate
    Each arrow is one machine, and each machine is a place where the line can starve.
  - throughput_tony | Y126-04-06 | I will add one. A molder at two and a half seconds can cast twenty-four ingots a minute. A copper furnace at five seconds makes twelve. So one molder serves two copper furnaces, and no more.
    - ratio_warden | Y126-04-06 | Cite the recipe and it is accepted.
    - decimal_doreen | Y126-04-06 | Twelve a minute into twenty-four a minute is two furnaces per molder, if the pour is instant. It is not instant, so one and a half is the honest figure. Two will take turns.
      - throughput_tony | Y126-04-07 | Two will take turns and lose nothing. The molder is never idle for long enough to matter.
        - decimal_doreen | Y126-04-07 | Then say "two will take turns", not "two is the ratio". They are different claims.
          - ratio_warden | Y126-04-07 | This exchange is the thread in miniature. Read it.

- old_slide_rule | Y127-09-12 | A caution from an old man. A ratio is a drawing of a factory that has never been built. Every ratio I did by hand in my working life was right on the paper and wrong on the floor, because the floor has men on it, and men take lunch.
  - ratio_warden | Y127-09-12 | That is true and it does not excuse a wrong number. A good ratio with a margin is better than a bad ratio with a story.
    - old_slide_rule | Y127-09-12 | I did not say otherwise. I said the floor has men on it.
      - foreman_ferro | Y135-08-11 | Six years later, this is still the best exchange on the board. A ratio you cannot staff is a drawing.

- [Deleted User #209] | Y128-02-14 | The numbers do not care whether you like them.
  - table_herr | Y128-02-14 | No. But they do care whether you have measured the right machine.
  - ratio_warden | Y128-02-14 | Both of you are right, which is unusual in this thread.

- unit_ursula | Y131-11-20 | A question from a teacher, asked in good faith. Why does the board use "per minute" and not "per second"? The recipes are in seconds.
  - ratio_warden | Y131-11-20 | Because a minute is the unit a person can feel. Five seconds means nothing to a foreman. Twelve a minute is a number he can compare with a belt.
    - unit_ursula | Y131-11-20 | Then the unit is a communication choice, not a mathematical one.
      - ratio_warden | Y131-11-20 | Every unit is a communication choice. That is why we are strict about them.
        - table_herr | Y131-11-21 | And why I draw the arrows. A picture is a unit too.

- pinch_point | Y134-02-08 | A method that has served me. Do not calculate the whole line. Calculate the slowest machine and build everything else to feed it. The slowest machine sets the rate and everything upstream is either enough or not enough.
  - ratio_warden | Y134-02-08 | Accepted, with a note. The slowest machine is not always the one with the longest recipe. A machine waiting on an input is slower than its recipe.
    - pinch_point | Y134-02-08 | That is fair. I should have said "the slowest machine in practice".
      - throughput_tony | Y134-02-09 | And in practice you find it by counting, not by reading recipes. Put a crate on the output for a shift.
        - pinch_point | Y134-02-09 | Then we agree, and I have said something imprecise on the internet.

- table_herr | Y137-06-02 | A table for the copper chain, drawn as I would have drawn it at the bench. All figures per minute, from the recipes as written.

  | Machine | In | Out | Per minute |
  |---|---|---|---|
  | furnace | 1 copper ore | 1 molten copper | 12 |
  | molder | 1 molten copper | 1 ingot | 24 |
  | press (plate) | 1 ingot | 1 plate | 24 |
  | press (pipe) | 2 plate | 1 pipe | 24 |
  | press (wire) | 1 plate | 2 wire | 24 |

  The pipe line eats two plate for one pipe, so a plate press feeding a pipe press has half its output spare. That is the sort of thing a table shows and a paragraph hides.
  - ratio_warden | Y137-06-02 | Accepted. This is the standard table for the copper chain until somebody finds an error in it.
    - decimal_doreen | Y137-06-03 | There is no error. I checked it twice.
  - grey_belt | Y138-01-19 | The table says a press makes twenty-four plate a minute. Mine makes about twenty and I have never understood why.
    - table_herr | Y138-01-19 | Because your press is waiting for ingots, or your belt is short of EC, or you are counting a bad minute. The table is what the machine does when it is fed. Yours is what it does when it is not.
      - grey_belt | Y138-01-19 | It was the belt. It had no pole near it.
        - ivo_p | Y138-01-20 | Check the pole before the machine. I say it here as I say it there.

- half_belt_hana | Y141-04-11 | A reality check from somebody who actually runs a line. All of this is correct and none of it is what I deal with. My line does not run at its recipe rate because the drill is on a mixed floor and the belt is shared. The ratio tells me what to expect. The floor tells me what I get. Both are useful, and the ratio is not the truth.
  - ratio_warden | Y141-04-11 | "The ratio is not the truth" is a sentence I would have locked a thread for, five years ago. It is correct. The ratio is the truth of a line with no interruptions. Yours has interruptions.
    - half_belt_hana | Y141-04-11 | Then put that in the first post.
      - ratio_warden | Y141-04-12 | I have.

- margin_mo | Y146-07-09 | A contribution. When you calculate a line, add ten per cent and call it the lunch margin. Not because the arithmetic is wrong, but because a line at exactly its recipe rate has no room for a crate to fill or a man to stop for a minute. Every line I have built at a hundred per cent has run at ninety.
  - foreman_ferro | Y146-07-09 | Ten per cent is the number I use. Some use twenty. Nobody uses zero twice.
    - pinch_point | Y146-07-09 | I used zero twice and rebuilt both lines.
      - margin_mo | Y146-07-10 | Then the margin is not a fudge. It is a design parameter.

- ratio_warden | Y149-01-06 | A note after twenty-three years. This thread has been wrong twice. Once about the molder, once about the pipe press, and both times a person with a crate and a shift of counting corrected it. That is the method working. Bring your counts.
  - the_third_decimal | Y149-01-06 | 23.4
    - ratio_warden | Y149-01-06 | Not that kind of count.
