# One iron line feeding two steel blast furnaces
board: Throughput   date: Y153-07-09   author: clean_melt   status: open

- clean_melt | Y153-07-09 | We are adding a second steel blast furnace and I want to know whether the iron line can feed both without a third furnace. Figures from the recipes, not from the floor.
  - decimal_doreen | Y153-07-09 | Work it backwards. A steel furnace on the coke recipe takes two iron plate and one coke every ten seconds: six steel a minute, and it eats twelve iron plate a minute. Each plate is one ingot, each ingot one molten iron. So one steel furnace wants twelve molten iron a minute.
    A blast furnace on iron makes one molten iron from one ore every two and a half seconds: twenty-four a minute. So one iron blast furnace feeds two steel furnaces exactly, with nothing to spare.
    - clean_melt | Y153-07-09 | Exactly is the word that worries me.
      - decimal_doreen | Y153-07-09 | It should. Exactly means no margin. Add a third iron furnace or accept that the second steel furnace runs at the rate of the first.
        - foreman_ferro | Y153-07-09 | And a third iron furnace wants twenty-four ore a minute, which is a belt. Do not add the furnace without the belt.
  - ferrous_ann | Y153-07-10 | The direct recipe changes the sum. Point the iron furnace into the steel furnace and it takes three molten iron and one coke for one steel. Three to one, not two plate to one steel, so the iron demand is different: three molten iron per steel, and one steel every ten seconds is eighteen molten iron a minute. One iron blast furnace makes twenty-four, so it feeds one and a third steel furnaces. Worse, not better, unless you are short of plate.
    - decimal_doreen | Y153-07-10 | Correct, and this is why the direct recipe is a capital saving and not a throughput saving. I will put it in the megathread.
      - ferrous_ann | Y153-07-10 | Put it in carefully. It is the most misunderstood recipe in the plant.
  - half_belt_hana | Y153-07-11 | On the floor, two steel furnaces on one iron furnace ran for a month and the second one was always the one that stopped. We added a third iron furnace and a crusher and stopped having the argument.
    - clean_melt | Y153-07-11 | A crusher?
      - half_belt_hana | Y153-07-11 | Grit gives more metal per lump. One ore to two grit, two grit to one iron. It buys you the margin that the arithmetic says you do not have.
        - decimal_doreen | Y153-07-11 | It does not buy margin, it buys metal. One ore whole gives two thirds of an iron in a furnace; as grit it gives one. That is half again, and half again is a margin.
          - half_belt_hana | Y153-07-11 | Then it buys margin.
  - pinch_point | Y153-07-12 | A warning about "exactly". Two machines matched exactly will drift, because neither runs at its recipe rate all day. Build the iron side a little over and the steel side a little under. That is not inefficiency, it is a line that runs.
    - ratio_warden | Y153-07-12 | Accepted, and it is the same note I have made four times in this board. Nobody has ever disagreed twice.
  - clean_melt | Y154-01-20 | Built it. Third iron furnace, crusher before it, and the second steel furnace now stops for coke rather than iron. Different problem, better problem.
    - decimal_doreen | Y154-01-20 | That is the correct outcome. The bottleneck moved, which means you fixed the right one.
