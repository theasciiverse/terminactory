# How do you feed a crude assembler with five different parts?
board: Build Orders & Layouts   date: Y143-11-07   author: young_hal   status: open

- young_hal | Y143-11-07 | I have my first crude assembler and it wants a valve: a copper pipe and an iron gear. That is two. A gearbox wants three gears and a steel, which is two kinds but four items. A relay wants three different things. How does anybody feed one of these without a belt for every part?
  - brass_tally | Y143-11-07 | You give it a belt for every part, and you give it a crate for every part, or you use one belt per recipe and change the recipe when you change the belt. There is no trick that avoids this. An assembler is a machine that waits for the slowest of its inputs.
    - young_hal | Y143-11-07 | That is a lot of belt.
      - brass_tally | Y143-11-07 | It is. It is also the machine that makes gearboxes, and gearboxes open fast belts and the basic assembler. You will only build the mess once.
  - crated_up | Y143-11-08 | Crates. One crate per input, an arm or a belt from each crate into the assembler, and a crate on the output. The crates do the waiting so the machines do not have to. I have four assemblers fed this way and they run all day.
    - young_hal | Y143-11-08 | Do the crates not fill up?
      - crated_up | Y143-11-08 | They fill and they stay full. A full crate is a buffer, not a fault. The assembler takes what it needs and the crate waits.
        - browncoat_bex | Y143-11-09 | And when you remove one, empty it first. I will say it again because it is the ninth time this month.
  - molder_max | Y143-11-09 | Another way, if the recipe shares inputs. A relay wants a coil, a spring and an iron sheet. A gearbox wants gears and steel. If you are making both, you need five inputs at the machine. If you are making only gearboxes, you need two. Build for one recipe, not for the whole book.
    - young_hal | Y143-11-09 | So one assembler per recipe?
      - molder_max | Y143-11-09 | One assembler per recipe you run continuously. If you change recipes twice a shift, you have a different problem.
  - tiny_lathe | Y143-11-10 | And check the speeds before you build the feeds. The crude assembler takes ten seconds for most of these. A press making gears takes two and a half seconds for one gear, and a gearbox wants three. If your gear line is one press, the assembler will be waiting on gears, not on steel.
    - young_hal | Y143-11-10 | So the assembler is not the slow part.
      - tiny_lathe | Y143-11-10 | The assembler is almost never the slow part. The parts are.
        - throughput_tony | Y143-11-11 | The assembler is the slowest machine in the building and it is still never the bottleneck, because its inputs are slower. This is the joke of every assembler line.
          - pinch_point | Y143-11-11 | It is not a joke, it is a queue. Find the slowest machine and feed it.
  - sand_and_stone | Y143-11-10 | Gears. Three gears a gearbox, and a gear is two iron plate. If you are feeding a gearbox line from one press you are feeding it from a press that takes two and a half seconds a gear. Count the gears before you count the assembler.
    - young_hal | Y143-11-10 | I have two presses on gears now.
      - sand_and_stone | Y143-11-10 | Then you have a line.

  - apprentice_nell | Y144-02-19 | I fed mine with four belts and no crates and it worked, but I had to stand and watch it. Then I put crates in and stopped watching it. Crates are the difference between a line and a job.
    - brass_tally | Y144-02-19 | Well put. A line that needs watching is not a line.
  - halvard_k | Y144-03-02 | Hello. Can a filtering arm take one item from a mixed crate and leave the rest?
    - crated_up | Y144-03-02 | Yes, set its filter. That is how I feed the fourth input without a fourth belt. One mixed crate, four arms, four filters.
      - halvard_k | Y144-03-02 | That is the trick I was looking for.
        - pipefitter_uri | Y144-03-03 | It is a good trick and it has one failure. If the mixed crate runs out of one item, the arm for that item stands still and you cannot see which crate is short. Four separate crates, four faults. One mixed crate, one mystery.
          - halvard_k | Y144-03-03 | Then I will use four crates and keep the mystery for other people.
