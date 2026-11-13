# How much cushion between a furnace and a molder?
board: Throughput   date: Y152-04-16   author: half_belt_hana   status: solved

- half_belt_hana | Y152-04-16 | Copper furnace into a molder with two tiles of belt between. It runs, then the molder starves for a second, then it runs. Is two tiles enough?
  - decimal_doreen | Y152-04-16 | One furnace makes twelve molten copper a minute and a molder casts twenty-four. The molder is idle half the time whatever you do. What you are seeing is not starvation, it is a machine waiting for its next pour.
    - half_belt_hana | Y152-04-16 | It says WAITING.
      - decimal_doreen | Y152-04-16 | It is waiting. That is the correct state for a molder on one furnace.
  - throughput_tony | Y152-04-17 | If the pause bothers you, put a second furnace on it. Two furnaces and one molder is the matched pair, and they take turns rather than wait.
    - half_belt_hana | Y152-04-17 | I do not have a second furnace.
      - throughput_tony | Y152-04-17 | Then you have a molder with spare capacity, which is better than a furnace with nowhere to pour.
  - table_herr | Y152-04-18 | A drawing: furnace 12 a minute, molder 24 a minute, so the molder is your spare. Draw the spare at the end of the line, not in the middle, and you will stop worrying about it.
    - half_belt_hana | Y152-04-18 | Understood. Edit: solved, and I am leaving the belt as it is.
