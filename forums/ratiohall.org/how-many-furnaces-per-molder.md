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

- halvard_k | Y143-04-11 | Hello. I run kilns and not molders, but I have been asked to ask this. A molder casting steel takes the same two and a half seconds as copper, and a steel blast furnace makes one steel every ten seconds. That is six a minute. So one molder is idle most of the time?
  - decimal_doreen | Y143-04-11 | Yes. One molder casts six steel a minute at best and can cast twenty-four. The molder is at a quarter of its rate.
    - halvard_k | Y143-04-11 | Then why does anybody build one?
      - decimal_doreen | Y143-04-11 | Because the steel has to become an ingot before it can be pressed, and six a minute is still steel.
  - ferrous_ann | Y143-04-12 | And if you point an iron blast furnace into a steel one, you do not need the molder for the iron at all. The steel still needs casting.
    - halvard_k | Y143-04-12 | That is the part I had not understood.
- molder_max | Y144-09-02 | I have run a molder for fourteen years and I will say the thing the arithmetic does not. A molder that is at a quarter of its rate is a molder that is never in a hurry, and a molder that is never in a hurry does not jam. I have had one jam in fourteen years and it was an ingot that came out crooked because the pour was rushed.
  - decimal_doreen | Y144-09-02 | That is a fact about a machine and it is not in the arithmetic.
    - molder_max | Y144-09-02 | Then put it beside the arithmetic.
      - decimal_doreen | Y144-09-02 | I have put it in the thread.
  - throughput_tony | Y144-09-03 | A molder at a quarter of its rate is also a molder you could point a second furnace at. Why have you not?
    - molder_max | Y144-09-03 | Because the second furnace is a second blast furnace and I do not have one.
- unit_ursula | Y145-05-19 | A teacher's question. If one molder can take two copper furnaces and only a quarter of a steel furnace, what is the number I should give a class for "furnaces per molder"?
  - decimal_doreen | Y145-05-19 | There is no single number. There is a table: copper two, iron two, blast iron one, steel a quarter.
    - unit_ursula | Y145-05-19 | That is a worse answer for a class and a better one for a person.
  - foreman_ferro | Y145-05-20 | Give them the copper number and tell them it is the copper number. They will find the rest when they build the rest.
- table_herr | Y146-08-02 | A drawing, because the thread has grown and a table is easier to argue with.
  copper furnace --12/m--> molder (24/m) <--12/m-- copper furnace
  iron furnace --12/m--> molder (24/m) <--12/m-- iron furnace
  blast iron --24/m--> molder (24/m)
  steel blast --6/m--> molder (24/m, mostly waiting)
  A molder is a machine with two mouths and one throat. Feed it from one side or two, but do not feed it three.
  - ratio_warden | Y146-08-02 | Accepted. This is the clearest thing on the thread.
  - decimal_doreen | Y146-08-03 | Two mouths and one throat. I will use that.
  - molder_max | Y146-08-03 | I have been saying it for fourteen years without the picture.
- pinch_point | Y147-11-06 | A correction to my own correction. I said one and a half furnaces per molder for a line that runs. For a steel molder the number is not one and a half of anything. The steel furnace is the slow machine and the molder is decoration. If you are counting steel, count the blast furnace and forget the molder.
  - decimal_doreen | Y147-11-06 | That is correct and it is a different kind of answer.
    - pinch_point | Y147-11-06 | It is the only kind that has ever helped me.
  - ferrous_ann | Y147-11-07 | The molder is where the steel becomes a thing you can sell. It is not decoration.
    - pinch_point | Y147-11-07 | It is not decoration and it is not the rate.
- throughput_tony | Y149-03-02 | I have a count that settles the copper question and I have been sitting on it for a year. One molder, two furnaces, a crate in front of each furnace and a crate in front of the molder. Ten hours: seven thousand one hundred and ninety ingots. That is twelve a minute, which is the furnace rate, not the molder rate.
  - decimal_doreen | Y149-03-02 | So the molder never idled and never filled.
    - throughput_tony | Y149-03-02 | The molder never idled and never filled and the crates did the waiting.
  - ratio_warden | Y149-03-03 | This is the first count on this thread and it is worth more than the arithmetic above it.
    - pinch_point | Y149-03-03 | It is worth more than my correction and I am the one who made the correction.
- half_belt_hana | Y151-07-18 | I have two furnaces and one molder and my ingots are short every afternoon. I have read the thread and I have done the arithmetic and the arithmetic says I am fine. What am I missing?
  - decimal_doreen | Y151-07-18 | Count the ore for one shift and the ingots for the same shift. If the ingots are short and the ore is not, the furnaces are waiting on the molder. If the ore is short, nothing downstream matters.
    - half_belt_hana | Y151-07-19 | The ore was short.
      - decimal_doreen | Y151-07-19 | Then you do not have two furnaces. You have one and a half and a drawing.
  - six_belts | Y151-07-19 | Six faults. Ore first.
- clean_melt | Y154-06-02 | A silicon note, since somebody will ask. A silicon molder casts at the same two and a half seconds and a silicon furnace does not feed it at anything like that rate. On a chip line we cast the silicon and we do not count the molder. We count the etcher.
  - ratio_warden | Y154-06-02 | Accepted. The molder is not the machine to count on silicon.
  - the_third_decimal | Y154-06-02 | 0.25.
    - decimal_doreen | Y154-06-02 | That is the steel figure and it is correct here by accident.
- decimal_doreen | Y157-02-11 | Twenty years of this thread and the answer has not changed. Two copper furnaces, two iron furnaces, one blast furnace on iron, and a quarter of a steel furnace. The only thing that has changed is that fewer people ask it as a trick question and more ask it to build something. That is an improvement.
  - ratio_warden | Y157-02-11 | It is the whole improvement.
  - molder_max | Y157-02-12 | One metal, one molder. It is still the best sentence on the board and it is still not a ratio.
    - decimal_doreen | Y157-02-12 | It is a rule of thumb and I have never had a better one.
- unit_ursula | Y159-05-14 | I taught this thread in a class last month and a student asked why the molder is the machine everybody counts when it is so often the machine that waits. I said because it is the machine everybody can see. I am putting it here because it is the truest thing I have said about ratios in twenty years.
  - foreman_ferro | Y159-05-14 | You count what you can see and you build what you cannot.
    - unit_ursula | Y159-05-14 | That is better than mine and I am taking it.
