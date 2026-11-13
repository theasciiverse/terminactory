# How many furnaces per molder?
board: Throughput   date: Y138-06-14   author: decimal_doreen   status: open

- decimal_doreen | Y138-06-14 | The question comes up monthly and the answer is always given without the arithmetic. Here is the arithmetic.
  A molder casts one ingot every two and a half seconds: twenty-four a minute.
  A furnace melts one copper ore to one molten copper every five seconds: twelve a minute.
  Therefore two copper furnaces per molder, and the molder is busy.
  An iron furnace makes two molten iron from three ore every ten seconds: twelve a minute. The same figure, by a different route. Two iron furnaces per molder.
  A blast furnace on iron makes one molten iron from one ore every two and a half seconds: twenty-four a minute. One blast furnace fills one molder on its own.
  - ratio_warden | Y138-06-14 | Accepted, with the standard caveat. The molder is never quite idle and never quite full. Two furnaces take turns. If you want a molder that never waits, use one furnace and accept a molder at half rate.
    - decimal_doreen | Y138-06-14 | That is a design choice, not a ratio. I have said the ratio; the choice is the reader's.
      - foreman_ferro | Y138-06-15 | And the choice on a real floor is nearly always two, because floor is cheaper than a molder and a man to watch it.
        - ratio_warden | Y138-06-15 | "Nearly always" is not a number. But it is true.
  - molder_max | Y138-06-15 | I run two furnaces into one molder and the molder has never stopped for a pour it could not take. I have watched it for six years.
    - throughput_tony | Y138-06-15 | Then you have a good arrangement and a poor stopwatch.
      - molder_max | Y138-06-15 | I have a good arrangement and six years of watching. I will take that over a stopwatch.
        - decimal_doreen | Y138-06-16 | Both of you are describing the same thing. The molder waits for the second furnace sometimes. It is two and a half seconds. Nobody notices two and a half seconds.

- halvard_k | Y139-01-22 | Hello. Does the molder's recipe matter? Steel takes the same two and a half seconds as copper.
  - decimal_doreen | Y139-01-22 | It does not. A molder casts any metal at the same rate. The difference is upstream: steel arrives at six a minute from one blast furnace, so the molder is mostly idle.
    - ferrous_ann | Y139-01-22 | And that is why a steel molder is a small machine in a big building. One blast furnace on steel makes six a minute. You could cast it in a molder and still have time to make tea.
      - ratio_warden | Y139-01-23 | Please do not write "time to make tea" in a throughput thread. Write "the molder is at a quarter of its rate."
        - ferrous_ann | Y139-01-23 | The molder is at a quarter of its rate.
          - ratio_warden | Y139-01-23 | Thank you.

- unit_ursula | Y140-05-08 | A teacher's question. If two furnaces per molder is the ratio, why does my line with two furnaces and one molder still starve the press?
  - decimal_doreen | Y140-05-08 | Because the press is the next machine and it takes one ingot every two and a half seconds, which is twenty-four a minute, and your two furnaces make twenty-four ingots a minute only when both are fed. If either furnace is short of ore, the press starves and it is not the molder's fault.
    - unit_ursula | Y140-05-08 | So the ratio is right and the line is still short.
      - decimal_doreen | Y140-05-08 | The ratio is right and the ore supply is short. Count the ore before you count the furnaces.
        - sand_and_stone | Y140-05-09 | And crush it. One ore becomes two grit and two grit melt to one iron, so a lump of ore gives half again as much metal. Your ore supply is not as short as you think if it is grit.
          - unit_ursula | Y140-05-09 | That changes the question entirely.
            - sand_and_stone | Y140-05-09 | It usually does.

- pinch_point | Y142-10-19 | A correction to the accepted answer, offered once and then dropped. Two furnaces per molder is correct for a molder that never waits and never idles. But a molder that is 100 per cent busy has no room for a hiccup, and a line with no room for a hiccup is a line that stops. One and a half is the honest number for a line that runs.
  - decimal_doreen | Y142-10-19 | I do not disagree. I gave the ratio; you are giving the margin.
    - pinch_point | Y142-10-19 | Then we are both right and the board can stop arguing.
      - ratio_warden | Y142-10-20 | The board will not stop arguing. But this thread is settled.
  - [Deleted User #1180] | Y143-03-07 | It evens out. Two furnaces, one molder, one press, and a crate in front of the press.
    - decimal_doreen | Y143-03-07 | The crate is not in the arithmetic but it is in the answer.
