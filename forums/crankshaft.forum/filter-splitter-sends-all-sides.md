# Filter splitter sends everything out the sides
board: Troubleshooting   date: Y142-04-11   author: grey_belt   status: solved

- grey_belt | Y142-04-11 | I have a belt of coal and ore coming from two drills into one line. I put a filter splitter on it, set to coal, and the coal still goes out the sides with everything else. The front belt is empty.
  - pipefitter_uri | Y142-04-11 | Which way is the splitter facing? It has a front, and the filtered item leaves by the front. If the front belt runs back towards the splitter, the splitter treats it as a side and puts nothing down it.
    - grey_belt | Y142-04-11 | The belt in front runs away from it. I checked twice.
      - pipefitter_uri | Y142-04-12 | Then check the filter is actually set. A filter splitter with no filter behaves exactly like a plain splitter and shares everything out. It does not warn you.
        - grey_belt | Y142-04-12 | It was set to copper ore. I had set it on the wrong machine and copied it along the row.
          - ivo_p | Y142-04-12 | Middle-clicking copies a filter, so if you set one wrong and copy it, you set them all wrong. I have done it across nine splitters in a row.
          - brass_tally | Y142-04-13 | Set the first one, look at it, then copy. Not the other way round.
  - tilda_m | Y142-04-13 | And a second filter splitter behind the first will sort the next item out, if you want coal, ore and stone separated. Series, not side by side. Side by side they all see the same belt and all take the first thing that arrives.
    - grey_belt | Y142-04-13 | Series. Understood. Edit: solved.
