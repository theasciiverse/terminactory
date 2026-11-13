# Iron melting is slower than copper and my plates starve
board: Troubleshooting   date: Y137-08-14   author: ferrous_ann   status: solved

- ferrous_ann | Y137-08-14 | Not a question, a fix, because I know three people are about to ask it. My iron line could not keep a press fed. The furnace takes three iron ore and ten seconds to give two molten iron, and the molder casts one ingot every two and a half seconds, so the press sat waiting most of the shift.
  What fixed it: a second iron furnace, and a crusher before both of them. Grit melts to more metal per lump and the two furnaces together keep the molder busy instead of taking turns.
  - molder_max | Y137-08-14 | Two furnaces into one molder is the arrangement I run. They do take turns, but the turns are short enough that the molder never waits.
    - ferrous_ann | Y137-08-15 | That is the difference. One furnace makes the molder wait. Two makes it work.
  - sand_and_stone | Y137-08-15 | The crusher is the part people skip. One iron ore crushes to two grit, and two grit melt to one iron. Whole ore gives you two iron from three lumps. Grit gives you one from one. It is not a small gain.
    - brass_tally | Y137-08-15 | And the crusher can push grit straight into a furnace in front of it, so you do not need the belt.
      - sand_and_stone | Y137-08-16 | I did not know that. I have been belting grit for a year.
        - brass_tally | Y137-08-16 | It is in the machine's description. We all read past it the first time.
  - ivo_p | Y137-08-16 | Check your EC before you add the second furnace's molder. The molder draws power and the second one is not free. A short grid slows everything on it evenly, which looks exactly like a slow furnace and is not one.
    - ferrous_ann | Y137-08-17 | Good point. I had a pole to spare. Edit: solved, and the line has run a week.
