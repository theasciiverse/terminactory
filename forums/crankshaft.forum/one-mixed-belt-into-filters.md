# What do you all think about one mixed belt into a bank of filter splitters?
board: Build Orders & Layouts   date: Y143-06-08   author: mateo_belts   status: open

- mateo_belts | Y143-06-08 | I have been building separate belts for every input since I started, and my floor is now mostly belt. A friend suggested one mixed belt carrying coal, ore and grit, with a bank of filter splitters taking each item off in turn. It would save me half the floor. What do you all think?
  - six_belts | Y143-06-08 | It works until one item is missing. Then the splitter that wanted it lets everything else past and your whole bank shifts one belt along. You will spend your life re-tuning it.
    - mateo_belts | Y143-06-08 | But the splitters are set to items, not to positions. If coal is missing, the coal splitter just passes the rest through.
      - six_belts | Y143-06-09 | And if the coal splitter has no coal, its front belt runs empty, which is fine. If the ore splitter has no ore, its front belt runs empty, which is fine. If the bank is fed by one drill that is low, all three run half empty and you cannot tell which is short. One belt, one fault. That is the whole of my argument.
        - mateo_belts | Y143-06-09 | That is a fair point about diagnosis.
          - brass_tally | Y143-06-09 | It is the only point that matters, in my view. A mixed belt is a puzzle you set for yourself at three in the morning.
  - pipefitter_uri | Y143-06-10 | I will defend the mixed belt, with conditions. If the mixed belt is short and the filters are in series and the last filter sends the remainder to a crate, it is a good arrangement for a small line. What kills it is length. A long mixed belt with a fault at the far end is a walk.
    - mateo_belts | Y143-06-10 | Short and in series. That is what I had in mind.
      - pipefitter_uri | Y143-06-10 | Then build it and tell us in a month.
  - OldHollowmere | Y143-06-10 | One belt, one fault. I have worked both ways and I know which way I slept better.
  - tilda_m | Y143-06-11 | Counting point. A mixed belt makes it very hard to say how much of anything you are moving. If you ever want to know your ore per minute, you will end up separating it again to find out.
    - throughput_tony | Y143-06-11 | This is the real objection. A mixed belt is unmeasurable. You cannot improve what you cannot count.
      - pinch_point | Y143-06-11 | You can count at the splitter. Put a crate on each output for ten minutes and weigh it.
        - throughput_tony | Y143-06-11 | That is not counting a line, that is an experiment. I want a number I can read off the plant.
          - pinch_point | Y143-06-12 | Then build separate belts and stop pretending the mixed one is the problem.

- molder_max | Y143-06-14 | I ran a mixed belt for two years. It was fine. It was fine because the line never changed. The week we added a second ore the whole bank had to be rebuilt. Separate belts cost floor and save you a rebuild.
  - mateo_belts | Y143-06-14 | That is the most convincing argument against, and it is not the one I expected.
  - crated_up | Y143-06-15 | Floor is cheap. Rebuilds are not.

- young_hal | Y143-06-16 | I am going to ask the stupid question. What is wrong with just using a splitter and letting it share? Why do the items need to be sorted at all?
  - unit_ursula | Y143-06-16 | Not stupid. A plain splitter shares everything out evenly, so each machine gets a mix of everything. A furnace that wants ore gets ore and coal and grit, and it will take the ore and stop at the rest. Sorting is how you give each machine only what it can use.
    - young_hal | Y143-06-16 | So a splitter is for the same item and a filter is for different items.
      - unit_ursula | Y143-06-17 | That is exactly it. Write that down.
      - six_belts | Y143-06-17 | That is better than my three paragraphs.

- ferrous_ann | Y143-06-20 | A mixed belt through a blast furnace block is a bad idea for one more reason. Waste comes out the back. If your input belt and your waste belt share a run, you will at some point send waste into a machine that cannot take it, and the machine will stop with a clear front and a clear back and you will lose an afternoon.
  - pipefitter_uri | Y143-06-20 | Good point. Never mix a waste run with an input run.
    - mateo_belts | Y143-06-21 | I had not thought of the waste at all.
      - ferrous_ann | Y143-06-21 | Nobody does until it happens.

- [Deleted User #4471] | Y143-06-22 | Separate belts tell you what is happening. A mixed belt tells you that something is happening. Those are different things and only one of them is useful.
  - six_belts | Y143-06-22 | I wish I had written that.
  - brass_tally | Y143-06-23 | So do I.

- mateo_belts | Y143-07-02 | An update, since people asked. I built the short mixed belt with three filters in series and a crate at the end. It ran for three weeks. It was fine. Then I added a second ore and rebuilt it in a shift, and the rebuild was not difficult because the belt was short.
  My conclusion: mixed belt for a small line that will not change. Separate belts for anything you intend to grow. I have gone back to separate belts, and I am happier, and my floor is a mess.
  - pipefitter_uri | Y143-07-02 | A fair report. Thank you for coming back with it.
  - six_belts | Y143-07-03 | One belt, one fault. You have earned the right to say it now.
  - pinch_point | Y143-07-03 | And you have a number for the rebuild, which is more than most people in this thread have.
    - mateo_belts | Y143-07-03 | One shift. Two people. I wrote it down.

- ratio_warden | Y144-01-15 | This thread is being linked from Ratio Hall as an example of a layout argument conducted properly. Note for future readers: nobody in it produced a throughput figure, and that is acceptable, because the question was about diagnosis and rebuild cost, not throughput. Do not quote it as evidence about rates.
  - throughput_tony | Y144-01-15 | I produced a figure. It was ignored.
    - ratio_warden | Y144-01-15 | You produced an opinion with a number in it.
